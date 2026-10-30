use super::reserve_bytes;

const MEBIBYTE: usize = 1024 * 1024;

#[test]
fn reserve_is_an_eighth_of_the_memory_between_one_and_sixty_four_mebibytes() {
    assert_eq!(reserve_bytes(19), MEBIBYTE);
    assert_eq!(reserve_bytes(2560), 20 * MEBIBYTE);
    assert_eq!(reserve_bytes(16_384), 64 * MEBIBYTE);
}

#[test]
fn reserve_of_a_full_address_space_does_not_overflow() {
    assert_eq!(reserve_bytes(65_536), 64 * MEBIBYTE);
}

#[test]
fn reserve_is_bounded_for_every_page_count() {
    assert_eq!(reserve_bytes(0), MEBIBYTE);
    assert_eq!(reserve_bytes(usize::MAX), 64 * MEBIBYTE);
}
