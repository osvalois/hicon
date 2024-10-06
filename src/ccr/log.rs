pub struct Log {
    entries: Vec<String>, // Simplified for this example
}

impl Log {
    pub fn new() -> Self {
        Log { entries: Vec::new() }
    }

    pub fn append(&mut self, entry: String) {
        self.entries.push(entry);
    }

    // Implement other log-related methods
}
