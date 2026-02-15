#![allow(non_upper_case_globals, non_camel_case_types, non_snake_case, dead_code)]

include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_struct_sizes() {
        // Ensure C structs have reasonable sizes (not zero)
        assert!(std::mem::size_of::<lh_position_t>() > 0);
        assert!(std::mem::size_of::<lh_web_color_t>() > 0);
        assert!(std::mem::size_of::<lh_container_vtable_t>() > 0);
    }

    #[test]
    fn test_null_document() {
        unsafe {
            // Passing null should not crash
            lh_document_destroy(std::ptr::null_mut());
            assert_eq!(lh_document_width(std::ptr::null()), 0.0);
            assert_eq!(lh_document_height(std::ptr::null()), 0.0);
        }
    }
}
