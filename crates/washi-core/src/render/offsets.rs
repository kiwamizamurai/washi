pub fn utf16_to_byte(text: &str, utf16: usize) -> Option<usize> {
    let mut units = 0;
    for (byte, c) in text.char_indices() {
        if units == utf16 {
            return Some(byte);
        }
        units += c.len_utf16();
        if units > utf16 {
            return None;
        }
    }
    (units == utf16).then_some(text.len())
}

pub fn byte_to_utf16(text: &str, byte: usize) -> usize {
    let mut end = byte.min(text.len());
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    text[..end].encode_utf16().count()
}

pub fn byte_to_line_column(text: &str, byte: usize) -> (u32, u32) {
    let mut end = byte.min(text.len());
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    let before = &text[..end];
    let line = before.matches('\n').count() as u32 + 1;
    let line_start = before.rfind('\n').map_or(0, |i| i + 1);
    (line, before[line_start..].chars().count() as u32 + 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ascii_is_the_identity() {
        assert_eq!(utf16_to_byte("hello", 3), Some(3));
        assert_eq!(byte_to_utf16("hello", 3), 3);
        assert_eq!(utf16_to_byte("hello", 5), Some(5));
        assert_eq!(utf16_to_byte("hello", 6), None);
    }

    #[test]
    fn japanese_is_one_utf16_unit_and_three_bytes() {
        let text = "日本語#lore";
        assert_eq!(utf16_to_byte(text, 3), Some(9));
        assert_eq!(byte_to_utf16(text, 9), 3);
        assert_eq!(utf16_to_byte(text, 4), Some(10));
        assert_eq!(utf16_to_byte(text, 8), Some(text.len()));
    }

    #[test]
    fn emoji_is_two_utf16_units_and_four_bytes() {
        let text = "a😀b";
        assert_eq!(utf16_to_byte(text, 1), Some(1));
        assert_eq!(utf16_to_byte(text, 3), Some(5));
        assert_eq!(utf16_to_byte(text, 4), Some(6));
        assert_eq!(byte_to_utf16(text, 5), 3);
        assert_eq!(byte_to_utf16(text, 6), 4);
    }

    #[test]
    fn a_position_inside_a_surrogate_pair_is_rejected() {
        assert_eq!(utf16_to_byte("a😀b", 2), None);
    }

    #[test]
    fn a_byte_inside_a_character_rounds_down() {
        assert_eq!(byte_to_utf16("日本", 4), 1);
        assert_eq!(byte_to_utf16("日本", 100), 2);
    }

    #[test]
    fn line_and_column_count_code_points_from_one() {
        let text = "ab\n日😀c\nz";
        assert_eq!(byte_to_line_column(text, 0), (1, 1));
        assert_eq!(byte_to_line_column(text, 3), (2, 1));
        assert_eq!(byte_to_line_column(text, 3 + 3 + 4), (2, 3));
        assert_eq!(byte_to_line_column(text, text.len()), (3, 2));
    }
}
