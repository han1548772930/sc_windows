/// OCR lifecycle phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Phase {
    /// No OCR work in progress.
    #[default]
    Idle,
    /// An OCR job has been started and is expected to complete asynchronously.
    Running,
}

/// Minimal OCR model.
#[derive(Debug, Default)]
pub struct Model {
    phase: Phase,
}

impl Model {
    pub fn phase(&self) -> Phase {
        self.phase
    }

    pub fn is_running(&self) -> bool {
        self.phase == Phase::Running
    }

    pub fn start(&mut self) {
        self.phase = Phase::Running;
    }

    pub fn finish(&mut self) {
        self.phase = Phase::Idle;
    }

    pub fn cancel(&mut self) {
        self.phase = Phase::Idle;
    }
}
