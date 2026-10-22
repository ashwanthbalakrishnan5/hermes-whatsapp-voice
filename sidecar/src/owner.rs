//! Owner identity is accepted only from the authenticated query of the configured PN.
use anyhow::{Result, bail};
use whatsapp_rust::prelude::*;

pub fn verified_identity(
    registered: bool,
    pn: &Jid,
    lid: Option<&Jid>,
    protocol_error: bool,
) -> Result<(Jid, Jid)> {
    if !registered || protocol_error || !pn.is_pn() {
        bail!("owner lookup did not return a registered PN identity");
    }
    let Some(lid) = lid.filter(|jid| jid.is_lid()) else {
        bail!("owner lookup did not return a verified LID");
    };
    Ok((pn.to_non_ad(), lid.to_non_ad()))
}

pub fn owner_matches(configured: &Jid, pn: &Jid, lid: &Jid, candidates: &[&Jid]) -> bool {
    !candidates.is_empty()
        && candidates.iter().all(|jid| {
            let jid = jid.to_non_ad();
            jid == *configured || jid == *pn || jid == *lid
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    // Synthetic fixtures only; not real WhatsApp identities.
    #[test]
    fn authenticated_canonical_pn_preserves_lid_identity() {
        let identity = verified_identity(
            true,
            &Jid::pn("15550000102"),
            Some(&Jid::lid("100000000001")),
            false,
        )
        .unwrap();
        assert_eq!(identity.0, Jid::pn("15550000102"));
        assert_eq!(identity.1, Jid::lid("100000000001"));
    }
    #[test]
    fn canonical_owner_is_authorized() {
        let configured = Jid::pn("15550000101");
        let pn = Jid::pn("15550000102");
        let lid = Jid::lid("100000000001");
        assert!(owner_matches(&configured, &pn, &lid, &[&pn, &lid]));
        assert!(owner_matches(&configured, &pn, &lid, &[&configured, &lid]));
        assert!(!owner_matches(&configured, &pn, &lid, &[]));
        assert!(!owner_matches(
            &configured,
            &pn,
            &lid,
            &[&pn, &Jid::lid("100000000002")]
        ));
        assert!(!owner_matches(
            &configured,
            &pn,
            &lid,
            &[&lid, &Jid::pn("15550000103")]
        ));
    }
    #[test]
    fn missing_lid_fails_closed() {
        assert!(verified_identity(true, &Jid::pn("15550000102"), None, false).is_err());
    }
    #[test]
    fn unregistered_or_protocol_error_fails_closed() {
        let pn = Jid::pn("15550000102");
        let lid = Jid::lid("100000000001");
        assert!(verified_identity(false, &pn, Some(&lid), false).is_err());
        assert!(verified_identity(true, &pn, Some(&lid), true).is_err());
    }
    #[test]
    fn wrong_namespaces_fail_closed() {
        assert!(
            verified_identity(
                true,
                &Jid::lid("100000000001"),
                Some(&Jid::pn("15550000102")),
                false
            )
            .is_err()
        );
    }
    #[test]
    fn verified_lid_owner_is_authorized() {
        let configured = Jid::pn("15550000101");
        let pn = Jid::pn("15550000102");
        let lid = Jid::lid("100000000001");
        assert!(owner_matches(&configured, &pn, &lid, &[&lid]));
    }
    #[test]
    fn unrelated_or_conflicting_identities_are_not_owner() {
        let configured = Jid::pn("15550000101");
        let pn = Jid::pn("15550000102");
        let lid = Jid::lid("100000000001");
        let other = Jid::pn("15550000103");
        let other_lid = Jid::lid("100000000002");
        assert!(!owner_matches(&configured, &pn, &lid, &[&other]));
        assert!(!owner_matches(&configured, &pn, &lid, &[&other_lid]));
        assert!(!owner_matches(&configured, &pn, &lid, &[&other, &lid]));
    }
}
