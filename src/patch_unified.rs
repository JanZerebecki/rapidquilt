use crate::line_interner::{LineId, LineInterner};
use crate::util::split_lines_with_endings;
const NO_NEW_LINE_TAG: &[u8] = b"\\ No newline at end of file\n";
    static ref MINUS_FILENAME: Regex = Regex::new(r"^--- ([^\t]+)\n$").unwrap();
    static ref PLUS_FILENAME: Regex = Regex::new(r"^\+\+\+ ([^\t]+)\n$").unwrap();
    static ref CHUNK: Regex = Regex::new(r"^@@ -(?P<remove_line>[\d]+)(?:,(?P<remove_count>[\d]+))? \+(?P<add_line>[\d]+)(?:,(?P<add_count>[\d]+))? @@?(?P<place_name>.*)\n$").unwrap();
    static ref DIFF_GIT: Regex = Regex::new(r"^diff --git +(?P<oldfilename>[^ ]+) +(?P<newfilename>[^ ]+)\n$").unwrap();
        r"^old mode +(?P<permissions>[0-9]+)\n$",
        r"^new mode +(?P<permissions>[0-9]+)\n$",
        r"^deleted file mode +(?P<permissions>[0-9]+)\n$",
        r"^new file mode +(?P<permissions>[0-9]+)\n$",
    let mut lines = split_lines_with_endings(bytes).peekable();
                let line = if line == b"\n" {
                    b" \n"
                // Check for the "No newline..." tag
                if lines.peek() == Some(&NO_NEW_LINE_TAG) && line_content.last() == Some(&b'\n') {
                    // Cut away the '\n' from the end of the line. It does not belong to the content,
                    // it is just there for patch formating.
                    line_content = &line_content[..(line_content.len()-1)];

                    // Skip the line with the tag
                    lines.next();
                }

        let add_count = self.add.content.len();
        let remove_count = self.remove.content.len();
        let add_line = if add_count == 0 {
        let remove_line = if remove_count == 0 {
        let mut write_line = |c: u8, line_id: LineId| -> Result<(), Error> {
            let line = interner.get(line_id).unwrap(); // NOTE(unwrap): Must succeed, we are printing patch that was already interned. If it is not there, it is a bug.
            writer.write(&[c])?;
            writer.write(line)?;
            if line.last() != Some(&b'\n') {
                // If the line doesn't end with newline character, we have to write it ourselves
                // (otherwise it would not be valid patch file), but we also print the "No newline..."
                // tag which informs that the newline is not part of the file.
                writer.write(NO_NEW_LINE_TAG)?;
                write_line(b'-', remove[remove_i])?;
                write_line(b'+', add[add_i])?;
                write_line(b' ', remove[remove_i])?;