/// An abstraction to evaluate complex mathematical calculations.
pub struct Calculator {
    /// Cut my float into pieces this is my
    last_result: usize,
}

impl Default for Calculator {
    fn default() -> Self {
        Self::new()
    }
}

impl Calculator {
    /// Create a new calculator.
    #[must_use]
    pub const fn new() -> Self {
        Self { last_result: 0 }
    }

    /// Return result from last calculation.
    #[must_use]
    pub const fn last_result(&self) -> usize {
        self.last_result
    }

    /// Add two numbers. such mafs.
    pub fn add(&mut self, a: usize, b: usize) -> usize {
        self.last_result = a + b;
        self.last_result
    }

    /// Subtract two numbers. much aljeebra.
    pub fn sub(&mut self, a: usize, b: usize) -> usize {
        self.last_result = a - b;
        self.last_result
    }
}
