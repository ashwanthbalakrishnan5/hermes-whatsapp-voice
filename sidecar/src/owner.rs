//! Owner identity. WhatsApp may present the owner as a LID, or as a PN that differs from the
//! configured one, so a caller is matched against the configured PN first and, only on a
//! mismatch, against the canonical PN/LID pair that WhatsApp returns for the configured PN.
use std::future::Future;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use anyhow::{Result, bail};
use whatsapp_rust::prelude::*;

/// Minimum gap between lookups, so unrelated callers cannot make the sidecar query WhatsApp
/// on every inbound call.
const LOOKUP_COOLDOWN: Duration = Duration::from_secs(10);

/// Validates an authenticated lookup of the configured PN into a canonical (PN, LID) pair.
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

#[derive(Default)]
struct State {
    pair: Option<(Jid, Jid)>,
    last_lookup: Option<Instant>,
}

pub struct Owner {
    configured: Jid,
    cooldown: Duration,
    state: Mutex<State>,
}

impl Owner {
    pub fn new(configured: Jid) -> Self {
        Self {
            configured,
            cooldown: LOOKUP_COOLDOWN,
            state: Mutex::default(),
        }
    }

    #[cfg(test)]
    fn with_cooldown(configured: Jid, cooldown: Duration) -> Self {
        Self {
            cooldown,
            ..Self::new(configured)
        }
    }

    fn all_owned(&self, candidates: &[&Jid], pair: Option<&(Jid, Jid)>) -> bool {
        !candidates.is_empty()
            && candidates.iter().all(|jid| {
                let jid = jid.to_non_ad();
                jid == self.configured || pair.is_some_and(|(pn, lid)| jid == *pn || jid == *lid)
            })
    }

    fn cached(&self) -> Option<(Jid, Jid)> {
        self.state.lock().unwrap().pair.clone()
    }

    /// Looks the owner up (at most once per cooldown) and caches a valid result. A failed lookup
    /// keeps the previous pair. Returns the pair now cached.
    async fn refresh<F, Fut>(&self, lookup: F) -> Option<(Jid, Jid)>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<(Jid, Jid)>>,
    {
        {
            let mut state = self.state.lock().unwrap();
            let now = Instant::now();
            if state
                .last_lookup
                .is_some_and(|at| now.duration_since(at) < self.cooldown)
            {
                return state.pair.clone();
            }
            state.last_lookup = Some(now);
        }
        match lookup().await {
            Ok(pair) => self.state.lock().unwrap().pair = Some(pair),
            Err(error) => log::warn!("owner lookup failed: {error:#}"),
        }
        self.cached()
    }

    /// Whether every supplied identity belongs to the owner (none may conflict or be unrelated).
    /// The configured PN is accepted without a lookup; `lookup` (WhatsApp's authenticated answer
    /// for the configured PN) runs only when the cached pair does not already cover them.
    pub async fn matches<F, Fut>(&self, candidates: &[&Jid], lookup: F) -> bool
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<(Jid, Jid)>>,
    {
        if candidates.is_empty() || self.all_owned(candidates, None) {
            return !candidates.is_empty();
        }
        if self.all_owned(candidates, self.cached().as_ref()) {
            return true;
        }
        let pair = self.refresh(lookup).await;
        self.all_owned(candidates, pair.as_ref())
    }

    /// Where to dial the owner: the canonical LID when known, else the configured PN.
    pub async fn dial_target<F, Fut>(&self, lookup: F) -> Jid
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<(Jid, Jid)>>,
    {
        let pair = match self.cached() {
            Some(pair) => Some(pair),
            None => self.refresh(lookup).await,
        };
        pair.map_or_else(|| self.configured.clone(), |(_, lid)| lid)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    // Synthetic fixtures only; not real WhatsApp identities.
    fn configured() -> Jid {
        Jid::pn("15550000101")
    }
    fn canonical_pn() -> Jid {
        Jid::pn("15550000102")
    }
    fn owner_lid() -> Jid {
        Jid::lid("100000000001")
    }
    fn pair() -> (Jid, Jid) {
        (canonical_pn(), owner_lid())
    }
    fn owner() -> Owner {
        Owner::with_cooldown(configured(), Duration::ZERO)
    }

    /// A lookup that counts its calls and returns `result` (an error when None).
    struct Lookups {
        calls: AtomicUsize,
        result: Option<(Jid, Jid)>,
    }
    impl Lookups {
        fn ok() -> Self {
            Self {
                calls: AtomicUsize::new(0),
                result: Some(pair()),
            }
        }
        fn failing() -> Self {
            Self {
                calls: AtomicUsize::new(0),
                result: None,
            }
        }
        async fn run(&self) -> Result<(Jid, Jid)> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            self.result
                .clone()
                .ok_or_else(|| anyhow::anyhow!("lookup failed"))
        }
        fn count(&self) -> usize {
            self.calls.load(Ordering::SeqCst)
        }
    }

    #[tokio::test]
    async fn exact_configured_pn_is_accepted_without_lookup() {
        let (o, l) = (owner(), Lookups::failing());
        let device = configured().with_device(3);
        assert!(o.matches(&[&configured()], || l.run()).await);
        assert!(o.matches(&[&configured(), &device], || l.run()).await);
        assert_eq!(l.count(), 0);
    }

    #[tokio::test]
    async fn no_identities_are_rejected_without_lookup() {
        let (o, l) = (owner(), Lookups::ok());
        assert!(!o.matches(&[], || l.run()).await);
        assert_eq!(l.count(), 0);
    }

    #[tokio::test]
    async fn owner_lid_is_matched_through_one_lookup_then_the_cache() {
        let (o, l) = (owner(), Lookups::ok());
        assert!(o.matches(&[&owner_lid()], || l.run()).await);
        assert_eq!(l.count(), 1);
        assert!(o.matches(&[&owner_lid()], || l.run()).await);
        assert!(
            o.matches(&[&canonical_pn(), &owner_lid()], || l.run())
                .await
        );
        assert_eq!(l.count(), 1, "cached pair answers repeat calls");
    }

    #[tokio::test]
    async fn mixed_configured_pn_and_lid_require_the_authenticated_pair() {
        let o = owner();
        let failing = Lookups::failing();
        assert!(
            !o.matches(&[&configured(), &owner_lid()], || failing.run())
                .await
        );
        let good = Lookups::ok();
        assert!(
            o.matches(&[&configured(), &owner_lid()], || good.run())
                .await
        );
        assert_eq!(good.count(), 1);
        assert!(
            o.matches(&[&configured(), &owner_lid()], || failing.run())
                .await
        );
        assert_eq!(failing.count(), 1, "the verified cache skips the lookup");
    }

    #[tokio::test]
    async fn lookup_failure_rejects_a_non_configured_identity() {
        let (o, l) = (owner(), Lookups::failing());
        assert!(!o.matches(&[&owner_lid()], || l.run()).await);
        assert!(!o.matches(&[&canonical_pn()], || l.run()).await);
        assert_eq!(l.count(), 2);
    }

    #[tokio::test]
    async fn conflicting_or_unrelated_identities_are_rejected() {
        let (o, l) = (owner(), Lookups::ok());
        let other_pn = Jid::pn("15550000103");
        let other_lid = Jid::lid("100000000002");
        assert!(!o.matches(&[&other_pn], || l.run()).await);
        assert!(!o.matches(&[&other_lid], || l.run()).await);
        assert!(!o.matches(&[&owner_lid(), &other_pn], || l.run()).await);
        assert!(!o.matches(&[&configured(), &other_lid], || l.run()).await);
        assert!(!o.matches(&[&owner_lid(), &other_lid], || l.run()).await);
    }

    #[tokio::test]
    async fn stale_cache_is_refreshed_on_a_miss() {
        let o = owner();
        let old = Lookups::ok();
        assert!(o.matches(&[&owner_lid()], || old.run()).await);
        let new_lid = Jid::lid("100000000009");
        let fresh = Lookups {
            calls: AtomicUsize::new(0),
            result: Some((canonical_pn(), new_lid.clone())),
        };
        assert!(o.matches(&[&new_lid], || fresh.run()).await);
        assert_eq!(fresh.count(), 1);
        assert!(!o.matches(&[&owner_lid()], || fresh.run()).await);
    }

    #[tokio::test]
    async fn failed_refresh_keeps_the_cached_pair() {
        let o = owner();
        let good = Lookups::ok();
        assert!(o.matches(&[&owner_lid()], || good.run()).await);
        let broken = Lookups::failing();
        assert!(
            !o.matches(&[&Jid::lid("100000000002")], || broken.run())
                .await
        );
        assert!(o.matches(&[&owner_lid()], || broken.run()).await);
    }

    #[tokio::test]
    async fn lookups_are_rate_limited() {
        let o = Owner::with_cooldown(configured(), Duration::from_secs(3600));
        let l = Lookups::failing();
        assert!(!o.matches(&[&Jid::lid("100000000002")], || l.run()).await);
        assert!(!o.matches(&[&Jid::lid("100000000003")], || l.run()).await);
        assert_eq!(
            l.count(),
            1,
            "second mismatch inside the cooldown does not query"
        );
    }

    #[tokio::test]
    async fn dial_target_prefers_the_canonical_lid() {
        let (o, l) = (owner(), Lookups::ok());
        assert_eq!(o.dial_target(|| l.run()).await, owner_lid());
        assert_eq!(o.dial_target(|| l.run()).await, owner_lid());
        assert_eq!(l.count(), 1);
    }

    #[tokio::test]
    async fn dial_target_falls_back_to_the_configured_pn() {
        let (o, l) = (owner(), Lookups::failing());
        assert_eq!(o.dial_target(|| l.run()).await, configured());
    }

    #[test]
    fn authenticated_canonical_pn_preserves_lid_identity() {
        let identity = verified_identity(true, &canonical_pn(), Some(&owner_lid()), false).unwrap();
        assert_eq!(identity, pair());
    }

    #[test]
    fn missing_lid_fails_closed() {
        assert!(verified_identity(true, &canonical_pn(), None, false).is_err());
    }

    #[test]
    fn unregistered_or_protocol_error_fails_closed() {
        let (pn, lid) = pair();
        assert!(verified_identity(false, &pn, Some(&lid), false).is_err());
        assert!(verified_identity(true, &pn, Some(&lid), true).is_err());
    }

    #[test]
    fn wrong_namespaces_fail_closed() {
        let (pn, lid) = pair();
        assert!(verified_identity(true, &lid, Some(&pn), false).is_err());
        assert!(verified_identity(true, &pn, Some(&pn), false).is_err());
    }
}
