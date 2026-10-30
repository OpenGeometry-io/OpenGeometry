pub fn compared_when(ci: Option<&str>) -> bool {
    ci != Some("true")
}

pub fn compared() -> bool {
    compared_when(std::env::var("CI").ok().as_deref())
}
