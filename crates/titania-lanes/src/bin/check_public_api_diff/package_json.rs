pub(crate) fn extract_package_names(json: &str) -> Vec<String> {
    let Some((start, remainder)) = package_remainder(json) else {
        return Vec::new();
    };
    scan_package_objects(start, remainder)
        .into_iter()
        .filter_map(|(object_start, object_end)| pull_name_field(json, object_start, object_end))
        .collect()
}

fn package_remainder(json: &str) -> Option<(usize, &str)> {
    let key = "\"packages\"";
    let key_pos = find_substring(json, key)?;
    let after_key = json.get(key_pos.saturating_add(key.len())..)?;
    let bracket_pos = find_byte(after_key, b'[')?;
    let start = key_pos.saturating_add(key.len()).saturating_add(bracket_pos);
    let remainder = json.get(start..)?;
    Some((start, remainder))
}

#[derive(Default)]
struct ObjectScan {
    depth: usize,
    in_string: bool,
    escape: bool,
    object_start: Option<usize>,
}

fn scan_package_objects(start: usize, remainder: &str) -> Vec<(usize, usize)> {
    let mut objects = Vec::new();
    let mut scan = ObjectScan::default();
    for (offset, byte) in remainder.bytes().enumerate() {
        let abs = start.saturating_add(offset);
        if scan.feed(byte, abs, &mut objects) {
            break;
        }
    }
    objects
}

impl ObjectScan {
    fn feed(&mut self, byte: u8, abs: usize, objects: &mut Vec<(usize, usize)>) -> bool {
        if self.in_string {
            self.feed_string(byte);
            return false;
        }
        match byte {
            b'"' => self.in_string = true,
            b'{' => self.open_object(abs),
            b'}' => self.close_object(abs, objects),
            b']' if self.depth == 0 => return true,
            _ => {}
        }
        false
    }

    fn feed_string(&mut self, byte: u8) {
        if self.escape {
            self.escape = false;
        } else if byte == b'\\' {
            self.escape = true;
        } else if byte == b'"' {
            self.in_string = false;
        }
    }

    fn open_object(&mut self, abs: usize) {
        if self.depth == 0 {
            self.object_start = Some(abs);
        }
        self.depth = self.depth.saturating_add(1);
    }

    fn close_object(&mut self, abs: usize, objects: &mut Vec<(usize, usize)>) {
        self.depth = self.depth.saturating_sub(1);
        if self.depth != 0 {
            return;
        }
        if let Some(object_start) = self.object_start {
            objects.push((object_start, abs));
        }
        self.object_start = None;
    }
}

fn find_substring(haystack: &str, needle: &str) -> Option<usize> {
    if needle.is_empty() || haystack.len() < needle.len() {
        return None;
    }
    haystack.as_bytes().windows(needle.len()).position(|w| w == needle.as_bytes())
}

fn find_byte(haystack: &str, byte: u8) -> Option<usize> {
    haystack.bytes().position(|b| b == byte)
}

fn pull_name_field(json: &str, object_start: usize, object_end: usize) -> Option<String> {
    let key = "\"name\"";
    let object = json.get(object_start..=object_end)?;
    let key_pos = find_substring(object, key)?;
    let after_key = object.get(key_pos.saturating_add(key.len())..)?;
    let colon_pos = find_byte(after_key, b':')?;
    let value_start = colon_pos.saturating_add(1);
    let after_colon = after_key.get(value_start..)?;
    let first_quote = find_byte(after_colon, b'"')?;
    let value_bytes_start = first_quote.saturating_add(1);
    let value_text = after_colon.get(value_bytes_start..)?;
    let end_quote = find_byte(value_text, b'"')?;
    value_text.get(..end_quote).map(String::from)
}
