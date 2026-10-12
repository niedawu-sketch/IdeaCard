use std::{
    collections::HashMap,
    fmt::{self, Debug, Formatter},
    sync::Mutex,
};

use crate::app_error::AppError;

pub const KEYCHAIN_SERVICE: &str = "com.niedawu.ideacard.ai";

pub trait SecretStore: Send + Sync {
    fn set(&self, account: &str, value: &str) -> Result<(), AppError>;
    fn exists(&self, account: &str) -> Result<bool, AppError>;
    fn get(&self, account: &str) -> Result<String, AppError>;
    fn remove(&self, account: &str) -> Result<(), AppError>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeychainOperation {
    Set,
    Get,
    Delete,
}

#[derive(Clone, PartialEq, Eq)]
pub struct KeychainCommand {
    pub operation: KeychainOperation,
    pub service: &'static str,
    pub account: String,
    secret: Option<String>,
}

impl KeychainCommand {
    fn new(
        operation: KeychainOperation,
        account: &str,
        secret: Option<&str>,
    ) -> Result<Self, AppError> {
        validate_account(account)?;
        Ok(Self {
            operation,
            service: KEYCHAIN_SERVICE,
            account: account.to_owned(),
            secret: secret.map(str::to_owned),
        })
    }

    pub fn secret(&self) -> Option<&str> {
        self.secret.as_deref()
    }
}

impl Debug for KeychainCommand {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("KeychainCommand")
            .field("operation", &self.operation)
            .field("service", &self.service)
            .field("account", &self.account)
            .field("secret", &self.secret.as_ref().map(|_| "[REDACTED]"))
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct KeychainCommandResult {
    secret: Option<String>,
}

impl KeychainCommandResult {
    pub fn empty() -> Self {
        Self { secret: None }
    }

    pub fn with_secret(secret: impl Into<String>) -> Self {
        Self {
            secret: Some(secret.into()),
        }
    }

    fn into_secret(self) -> Option<String> {
        self.secret
    }
}

impl Debug for KeychainCommandResult {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("KeychainCommandResult")
            .field("secret", &self.secret.as_ref().map(|_| "[REDACTED]"))
            .finish()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeychainFailure {
    NotFound,
    Unavailable,
    Failed,
    InvalidResponse,
}

pub trait KeychainCommandAdapter: Send + Sync {
    fn execute(&self, command: KeychainCommand) -> Result<KeychainCommandResult, KeychainFailure>;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct SystemKeychainAdapter;

#[cfg(target_os = "macos")]
impl KeychainCommandAdapter for SystemKeychainAdapter {
    fn execute(&self, command: KeychainCommand) -> Result<KeychainCommandResult, KeychainFailure> {
        use security_framework::passwords::{
            delete_generic_password, generic_password, set_generic_password, PasswordOptions,
        };

        const ERR_SEC_ITEM_NOT_FOUND: i32 = -25_300;
        let map_error = |error: security_framework::base::Error| {
            if error.code() == ERR_SEC_ITEM_NOT_FOUND {
                KeychainFailure::NotFound
            } else {
                KeychainFailure::Failed
            }
        };
        match command.operation {
            KeychainOperation::Set => {
                let secret = command.secret().ok_or(KeychainFailure::InvalidResponse)?;
                set_generic_password(command.service, &command.account, secret.as_bytes())
                    .map_err(map_error)?;
                Ok(KeychainCommandResult::empty())
            }
            KeychainOperation::Get => {
                let secret = generic_password(PasswordOptions::new_generic_password(
                    command.service,
                    &command.account,
                ))
                .map_err(map_error)?;
                let secret =
                    String::from_utf8(secret).map_err(|_| KeychainFailure::InvalidResponse)?;
                Ok(KeychainCommandResult::with_secret(secret))
            }
            KeychainOperation::Delete => {
                delete_generic_password(command.service, &command.account).map_err(map_error)?;
                Ok(KeychainCommandResult::empty())
            }
        }
    }
}

#[cfg(not(target_os = "macos"))]
impl KeychainCommandAdapter for SystemKeychainAdapter {
    fn execute(&self, _command: KeychainCommand) -> Result<KeychainCommandResult, KeychainFailure> {
        Err(KeychainFailure::Unavailable)
    }
}

pub struct MacOsKeychain<A> {
    adapter: A,
    cache: Mutex<HashMap<String, String>>,
}

impl<A> MacOsKeychain<A> {
    pub fn new(adapter: A) -> Self {
        Self {
            adapter,
            cache: Mutex::new(HashMap::new()),
        }
    }

    #[cfg(test)]
    fn adapter(&self) -> &A {
        &self.adapter
    }
}

impl<A: KeychainCommandAdapter> MacOsKeychain<A> {
    fn read_once(&self, account: &str) -> Result<Option<String>, AppError> {
        validate_account(account)?;
        let mut cache = self.cache.lock().map_err(|_| keychain_state_error())?;
        if let Some(secret) = cache.get(account) {
            return Ok(Some(secret.clone()));
        }
        match self
            .adapter
            .execute(KeychainCommand::new(KeychainOperation::Get, account, None)?)
        {
            Ok(result) => {
                let secret = result.into_secret();
                if let Some(value) = secret.as_ref() {
                    cache.insert(account.to_owned(), value.clone());
                }
                Ok(secret)
            }
            Err(KeychainFailure::NotFound) => Ok(None),
            Err(failure) => Err(map_keychain_failure(failure)),
        }
    }
}

impl<A: KeychainCommandAdapter> SecretStore for MacOsKeychain<A> {
    fn set(&self, account: &str, value: &str) -> Result<(), AppError> {
        if value.trim().is_empty() {
            return Err(AppError::new(
                "ai_key_invalid",
                "API Key 不能为空",
                "重新输入 API Key",
            ));
        }
        self.adapter
            .execute(KeychainCommand::new(
                KeychainOperation::Set,
                account,
                Some(value),
            )?)
            .map_err(map_keychain_failure)?;
        self.cache
            .lock()
            .map_err(|_| keychain_state_error())?
            .insert(account.to_owned(), value.to_owned());
        Ok(())
    }

    fn exists(&self, account: &str) -> Result<bool, AppError> {
        Ok(self.read_once(account)?.is_some())
    }

    fn get(&self, account: &str) -> Result<String, AppError> {
        self.read_once(account)?.ok_or_else(missing_key_error)
    }

    fn remove(&self, account: &str) -> Result<(), AppError> {
        match self.adapter.execute(KeychainCommand::new(
            KeychainOperation::Delete,
            account,
            None,
        )?) {
            Ok(_) | Err(KeychainFailure::NotFound) => {
                self.cache
                    .lock()
                    .map_err(|_| keychain_state_error())?
                    .remove(account);
                Ok(())
            }
            Err(failure) => Err(map_keychain_failure(failure)),
        }
    }
}

fn validate_account(account: &str) -> Result<(), AppError> {
    if account.trim().is_empty() || account.len() > 100 {
        return Err(AppError::new(
            "ai_key_account_invalid",
            "API Key 账户标识无效",
            "重新打开应用后重试",
        ));
    }
    Ok(())
}

fn map_keychain_failure(failure: KeychainFailure) -> AppError {
    match failure {
        KeychainFailure::NotFound => missing_key_error(),
        KeychainFailure::Unavailable => AppError::new(
            "ai_keychain_unavailable",
            "无法访问 macOS 钥匙串",
            "检查钥匙串访问权限后重试",
        ),
        KeychainFailure::Failed | KeychainFailure::InvalidResponse => AppError::new(
            "ai_keychain_failed",
            "无法完成 API Key 操作",
            "重新打开应用后重试",
        ),
    }
}

fn missing_key_error() -> AppError {
    AppError::new(
        "ai_key_missing",
        "尚未保存 API Key",
        "先在 AI 设置中保存 API Key",
    )
}

fn keychain_state_error() -> AppError {
    AppError::new(
        "ai_keychain_state_failed",
        "无法读取本次启动中的 API Key",
        "重新打开应用后重试",
    )
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::{
        KeychainCommand, KeychainCommandAdapter, KeychainCommandResult, KeychainFailure,
        KeychainOperation, MacOsKeychain, SecretStore, KEYCHAIN_SERVICE,
    };

    const ACCOUNT: &str = "openAi";
    const SECRET: &str = "test-keychain-secret";

    struct FakeAdapter {
        result: Mutex<Result<KeychainCommandResult, KeychainFailure>>,
        commands: Mutex<Vec<KeychainCommand>>,
    }

    impl FakeAdapter {
        fn returning(result: Result<KeychainCommandResult, KeychainFailure>) -> Self {
            Self {
                result: Mutex::new(result),
                commands: Mutex::new(Vec::new()),
            }
        }
    }

    impl KeychainCommandAdapter for FakeAdapter {
        fn execute(
            &self,
            command: KeychainCommand,
        ) -> Result<KeychainCommandResult, KeychainFailure> {
            self.commands.lock().unwrap().push(command);
            self.result.lock().unwrap().clone()
        }
    }

    #[test]
    fn set_uses_the_fixed_service_and_redacts_command_debug() {
        let adapter = FakeAdapter::returning(Ok(KeychainCommandResult::empty()));
        let store = MacOsKeychain::new(adapter);

        store.set(ACCOUNT, SECRET).unwrap();

        let commands = store.adapter().commands.lock().unwrap();
        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0].operation, KeychainOperation::Set);
        assert_eq!(commands[0].service, KEYCHAIN_SERVICE);
        assert_eq!(commands[0].account, ACCOUNT);
        assert_eq!(commands[0].secret(), Some(SECRET));
        assert!(!format!("{:?}", commands[0]).contains(SECRET));
    }

    #[test]
    fn exists_distinguishes_present_and_missing_items() {
        let present = MacOsKeychain::new(FakeAdapter::returning(Ok(
            KeychainCommandResult::with_secret(SECRET),
        )));
        assert!(present.exists(ACCOUNT).unwrap());

        let missing = MacOsKeychain::new(FakeAdapter::returning(Err(KeychainFailure::NotFound)));
        assert!(!missing.exists(ACCOUNT).unwrap());
    }

    #[test]
    fn get_returns_the_secret_without_exposing_it_in_debug_output() {
        let store = MacOsKeychain::new(FakeAdapter::returning(Ok(
            KeychainCommandResult::with_secret(SECRET),
        )));

        assert_eq!(store.get(ACCOUNT).unwrap(), SECRET);
        let result = KeychainCommandResult::with_secret(SECRET);
        assert!(!format!("{result:?}").contains(SECRET));
    }

    #[test]
    fn exists_and_get_share_one_keychain_read_for_the_session() {
        let store = MacOsKeychain::new(FakeAdapter::returning(Ok(
            KeychainCommandResult::with_secret(SECRET),
        )));

        assert!(store.exists(ACCOUNT).unwrap());
        assert_eq!(store.get(ACCOUNT).unwrap(), SECRET);
        assert_eq!(store.adapter().commands.lock().unwrap().len(), 1);
    }

    #[test]
    fn remove_is_idempotent_when_item_does_not_exist() {
        let store = MacOsKeychain::new(FakeAdapter::returning(Err(KeychainFailure::NotFound)));

        store.remove(ACCOUNT).unwrap();
    }

    #[test]
    fn adapter_failures_map_to_stable_errors_without_secrets() {
        let store = MacOsKeychain::new(FakeAdapter::returning(Err(KeychainFailure::Unavailable)));

        let error = store.set(ACCOUNT, SECRET).unwrap_err();
        assert_eq!(error.code, "ai_keychain_unavailable");
        assert!(!format!("{error:?}").contains(SECRET));
    }
}
