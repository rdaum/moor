mod common;

#[test]
fn test_echo() -> eyre::Result<()> {
    common::run_test_as(&["wizard"], |mut client| {
        assert_eq!("42\n", client.command("; 42")?);
        Ok(())
    })
}
