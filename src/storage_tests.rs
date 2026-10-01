use super::*;

#[test]
fn windows_entry_open_recovers_after_a_replacement_conflict() {
    let mut errors = [5, 32, 303].into_iter();
    let mut waits = 0;
    let value = retry_windows_entry_open(
        || match errors.next() {
            Some(code) => Err(io::Error::from_raw_os_error(code)),
            None => Ok("complete entry"),
        },
        |_| waits += 1,
    )
    .unwrap();
    assert_eq!(value, "complete entry");
    assert_eq!(waits, 3);
}

#[test]
fn windows_entry_open_preserves_persistent_access_errors_after_bounded_retries() {
    for code in [5, 32, 303] {
        let mut attempts = 0;
        let mut waited = std::time::Duration::ZERO;
        let error = retry_windows_entry_open::<File>(
            || {
                attempts += 1;
                Err(io::Error::from_raw_os_error(code))
            },
            |delay| waited += delay,
        )
        .unwrap_err();
        assert_eq!(error.raw_os_error(), Some(code));
        assert_eq!(attempts, 4);
        assert!(waited <= std::time::Duration::from_millis(100));
    }
}

#[test]
fn windows_entry_open_returns_missing_and_other_errors_without_retrying() {
    for code in [2, 3, 6] {
        let error = retry_windows_entry_open::<File>(
            || Err(io::Error::from_raw_os_error(code)),
            |_| panic!("an unrelated error must not be retried"),
        )
        .unwrap_err();
        assert_eq!(error.raw_os_error(), Some(code));
    }
}
