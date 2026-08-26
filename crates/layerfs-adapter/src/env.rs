/// `"dnf"` -> `"LAYERFS_DNF_BIN"`.
pub fn bin_env_var(name: &str) -> String {
    format!("LAYERFS_{}_BIN", name.to_uppercase().replace('-', "_"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uppercases_and_replaces_dashes() {
        assert_eq!(bin_env_var("dnf"), "LAYERFS_DNF_BIN");
        assert_eq!(bin_env_var("apt-get"), "LAYERFS_APT_GET_BIN");
    }
}
