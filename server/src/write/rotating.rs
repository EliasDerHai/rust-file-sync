use chrono::Local;
use std::fs::{self, create_dir_all, OpenOptions};
use std::io::{self, Write};
use std::path::PathBuf;

/// A self-contained rotating file writer that manages multiple timestamped files.
/// Automatically rotates when file size exceeds limit and prunes old files (FIFO).
pub struct RotatingFileWriter {
    dir: PathBuf,
    prefix: String,
    max_size_bytes: u64,
    max_files: usize,
    current_file: Option<fs::File>,
    current_file_path: Option<PathBuf>,
    current_size: u64,
    headers: Option<String>,
}

impl RotatingFileWriter {
    /// Creates a new RotatingFileWriter.
    /// - `dir`: Directory to store rotated files
    /// - `prefix`: Filename prefix (e.g., "monitor" -> "monitor_2026-01-22T10-30-45.csv")
    /// - `max_size_bytes`: Maximum size per file before rotation
    /// - `max_files`: Maximum number of files to keep (FIFO pruning)
    /// - `headers`: Optional CSV headers to write at the start of each new file
    pub fn new(
        dir: PathBuf,
        prefix: String,
        max_size_bytes: u64,
        max_files: usize,
        headers: Option<String>,
    ) -> io::Result<Self> {
        // Ensure directory exists
        if !dir.exists() {
            create_dir_all(&dir)?;
        }

        let mut writer = Self {
            dir,
            prefix,
            max_size_bytes,
            max_files,
            current_file: None,
            current_file_path: None,
            current_size: 0,
            headers,
        };

        // Try to resume the most recent file if it exists and isn't full
        writer.resume_or_create()?;

        Ok(writer)
    }

    /// Lists existing files matching our pattern, sorted oldest to newest by filename.
    fn list_existing_files(&self) -> io::Result<Vec<PathBuf>> {
        let mut files: Vec<PathBuf> = fs::read_dir(&self.dir)?
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.path())
            .filter(|path| {
                path.is_file()
                    && path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .map(|n| n.starts_with(&self.prefix) && n.ends_with(".csv"))
                        .unwrap_or(false)
            })
            .collect();

        // Sort by filename (timestamps in filename ensure chronological order)
        files.sort();
        Ok(files)
    }

    /// Resumes writing to the most recent file if it's under the size limit,
    /// otherwise creates a new file.
    fn resume_or_create(&mut self) -> io::Result<()> {
        let files = self.list_existing_files()?;

        if let Some(newest) = files.last() {
            let metadata = fs::metadata(newest)?;
            let size = metadata.len();

            if size < self.max_size_bytes {
                // Resume this file
                let file = OpenOptions::new().append(true).open(newest)?;
                self.current_file = Some(file);
                self.current_file_path = Some(newest.clone());
                self.current_size = size;
                return Ok(());
            }
        }

        // No suitable file found, create a new one
        self.create_new_file()
    }

    /// Creates a new timestamped file and prunes old files if necessary.
    fn create_new_file(&mut self) -> io::Result<()> {
        // Prune if we're at capacity
        self.prune_old_files()?;

        // Generate timestamped filename
        let timestamp = Local::now().format("%Y-%m-%dT%H-%M-%S");
        let filename = format!("{}_{}.csv", self.prefix, timestamp);
        let path = self.dir.join(&filename);

        // Create file and write headers if provided
        let mut file = fs::File::create(&path)?;
        let mut initial_size = 0u64;

        if let Some(ref headers) = self.headers {
            writeln!(file, "{}", headers)?;
            initial_size = headers.len() as u64 + 1; // +1 for newline
        }

        self.current_file = Some(file);
        self.current_file_path = Some(path);
        self.current_size = initial_size;

        Ok(())
    }

    /// Removes oldest files if we have reached max_files capacity.
    fn prune_old_files(&mut self) -> io::Result<()> {
        let files = self.list_existing_files()?;

        // We need room for one new file, so delete until we have max_files - 1
        let files_to_delete = files.len().saturating_sub(self.max_files - 1);

        for path in files.into_iter().take(files_to_delete) {
            fs::remove_file(path)?;
        }

        Ok(())
    }

    /// Writes a line to the current file, rotating if necessary.
    pub fn write_line(&mut self, line: &str) -> io::Result<()> {
        let line_size = line.len() as u64 + 1; // +1 for newline

        // Check if we need to rotate before writing
        if self.current_size + line_size > self.max_size_bytes {
            self.rotate()?;
        }

        // Write the line
        if let Some(ref mut file) = self.current_file {
            writeln!(file, "{}", line)?;
            self.current_size += line_size;
        }

        Ok(())
    }

    /// Forces rotation to a new file.
    fn rotate(&mut self) -> io::Result<()> {
        // Close current file
        self.current_file = None;
        self.current_file_path = None;
        self.current_size = 0;

        // Create new file
        self.create_new_file()
    }
}
