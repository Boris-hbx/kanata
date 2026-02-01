/// Animated spinner for indicating in-progress operations.
///
/// Used to show visual feedback while the agent is thinking or a tool
/// is executing.
pub struct Spinner {
    frames: &'static [&'static str],
    current_frame: usize,
    message: String,
    active: bool,
}

impl Spinner {
    /// Create a new idle spinner with braille-dot animation frames.
    pub fn new() -> Self {
        Self {
            frames: &[
                "\u{280b}", "\u{2819}", "\u{2839}", "\u{2838}", "\u{283c}", "\u{2834}", "\u{2826}",
                "\u{2827}", "\u{2807}", "\u{280f}",
            ],
            current_frame: 0,
            message: String::new(),
            active: false,
        }
    }

    /// Start the spinner with a message.
    pub fn start(&mut self, message: impl Into<String>) {
        self.message = message.into();
        self.active = true;
        self.current_frame = 0;
    }

    /// Stop the spinner.
    pub fn stop(&mut self) {
        self.active = false;
    }

    /// Advance one frame. Returns the frame character if active.
    pub fn tick(&mut self) -> Option<&str> {
        if !self.active {
            return None;
        }
        let frame = self.frames[self.current_frame];
        self.current_frame = (self.current_frame + 1) % self.frames.len();
        Some(frame)
    }

    /// Return the current frame character without advancing, or empty string if idle.
    pub fn current_frame_char(&self) -> &str {
        if self.active { self.frames[self.current_frame] } else { "" }
    }

    /// Whether the spinner is currently running.
    pub fn is_active(&self) -> bool {
        self.active
    }

    /// The message displayed alongside the spinner.
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl Default for Spinner {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spinner_lifecycle() {
        let mut spinner = Spinner::new();
        assert!(!spinner.is_active());
        assert!(spinner.tick().is_none());

        spinner.start("Loading...");
        assert!(spinner.is_active());
        assert_eq!(spinner.message(), "Loading...");

        let frame = spinner.tick();
        assert!(frame.is_some());

        spinner.stop();
        assert!(!spinner.is_active());
        assert!(spinner.tick().is_none());
    }

    #[test]
    fn test_current_frame_char() {
        let mut spinner = Spinner::new();
        assert_eq!(spinner.current_frame_char(), "");

        spinner.start("Working...");
        let frame = spinner.current_frame_char();
        assert!(!frame.is_empty());

        spinner.stop();
        assert_eq!(spinner.current_frame_char(), "");
    }
}
