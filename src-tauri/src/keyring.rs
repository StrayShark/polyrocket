use keyring::Entry;

const SERVICE: &str = "com.polyrocket.wallet";

pub fn set_key(alias: &str, secret: &str) -> crate::AppResult<()> {
    let entry = Entry::new(SERVICE, alias)?;
    entry.set_password(secret)?;
    Ok(())
}

pub fn get_key(alias: &str) -> crate::AppResult<String> {
    let entry = Entry::new(SERVICE, alias)?;
    Ok(entry.get_password()?)
}

pub fn delete_key(alias: &str) -> crate::AppResult<()> {
    let entry = Entry::new(SERVICE, alias)?;
    entry.delete_credential()?;
    Ok(())
}

pub fn list_aliases() -> crate::AppResult<Vec<String>> {
    // The `keyring` crate doesn't expose enumeration. We track aliases
    // ourselves in SQLite (audit_log action='key.alias.add').
    Ok(vec![])
}