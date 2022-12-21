use cosmwasm_std::{StdError, StdResult};

pub fn assert_deadline(blocktime: u64, deadline: Option<u64>) -> StdResult<()> {
    if let Some(deadline) = deadline {
        if blocktime >= deadline {
            return Err(StdError::generic_err("Expired deadline"));
        }
    }

    Ok(())
}

#[test]
fn test_assert_deadline_with_normal() {
    assert_deadline(5u64, Some(10u64)).unwrap();
}

#[test]
fn test_assert_deadline_with_expired() {
    let err = assert_deadline(10u64, Some(5u64)).unwrap_err();
    assert_eq!(err, StdError::generic_err("Expired deadline"))
}

#[test]
fn test_assert_deadline_with_same() {
    let err = assert_deadline(10u64, Some(10u64)).unwrap_err();
    assert_eq!(err, StdError::generic_err("Expired deadline"))
}

#[test]
fn test_assert_deadline_with_none() {
    assert_deadline(5u64, None).unwrap();
}
