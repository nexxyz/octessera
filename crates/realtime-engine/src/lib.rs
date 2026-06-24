mod simd;

pub mod synth;

pub fn ping() -> &'static str {
    "ok"
}

#[cfg(test)]
mod tests {
    use super::ping;

    #[test]
    fn ping_works() {
        assert_eq!(ping(), "ok");
    }
}
