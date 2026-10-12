//! Accessibility (辅助功能) trust for synthetic scroll events.
//!
//! The main app owns the permission prompt on purpose: a prompt shown by the
//! main bundle registers a proper "IdeaCard" entry in the system
//! Accessibility list, and the `ideacard-native` helper process inherits that
//! trust as a child process. A prompt raised inside the bare helper binary
//! registers an entry users cannot find or add themselves.

#[cfg(target_os = "macos")]
mod imp {
    use core_foundation::base::TCFType;
    use core_foundation::boolean::CFBoolean;
    use core_foundation::dictionary::CFDictionary;
    use core_foundation::string::CFString;

    #[link(name = "ApplicationServices", kind = "framework")]
    extern "C" {
        fn AXIsProcessTrusted() -> bool;
        fn AXIsProcessTrustedWithOptions(
            options: core_foundation::dictionary::CFDictionaryRef,
        ) -> bool;
    }

    pub fn is_trusted() -> bool {
        unsafe { AXIsProcessTrusted() }
    }

    /// Shows the system prompt that jumps to the Accessibility settings and
    /// registers IdeaCard in the list. Returns the current trust state.
    pub fn prompt_for_trust() -> bool {
        // Same value as kAXTrustedCheckOptionPrompt.
        let key = CFString::new("AXTrustedCheckOptionPrompt");
        let value = CFBoolean::true_value();
        let options = CFDictionary::from_CFType_pairs(&[(key.as_CFType(), value.as_CFType())]);
        unsafe { AXIsProcessTrustedWithOptions(options.as_concrete_TypeRef()) }
    }
}

#[cfg(not(target_os = "macos"))]
mod imp {
    pub fn is_trusted() -> bool {
        false
    }

    pub fn prompt_for_trust() -> bool {
        false
    }
}

pub use imp::{is_trusted, prompt_for_trust};
