//! Tests for issue #353: contract-level supported SEP list and feature flags.

use anchorkit::contract::{
    AnchorKitContract, SepFeatureFlags, SEP_10, SEP_24, SEP_31, SEP_38, SEP_6,
};
use soroban_sdk::Env;

#[test]
fn supported_seps_returns_expected_list() {
    let env = Env::default();
    let seps = AnchorKitContract::supported_seps(env);
    assert_eq!(seps.len(), 5);
    assert!(seps.contains(&SEP_6));
    assert!(seps.contains(&SEP_10));
    assert!(seps.contains(&SEP_24));
    assert!(seps.contains(&SEP_31));
    assert!(seps.contains(&SEP_38));
}

#[test]
fn supported_seps_constants_have_correct_values() {
    assert_eq!(SEP_6, 6);
    assert_eq!(SEP_10, 10);
    assert_eq!(SEP_24, 24);
    assert_eq!(SEP_31, 31);
    assert_eq!(SEP_38, 38);
}

#[test]
fn supported_sep_feature_flags_all_enabled() {
    let env = Env::default();
    let flags: SepFeatureFlags = AnchorKitContract::supported_sep_feature_flags(env);
    assert!(flags.sep6);
    assert!(flags.sep10);
    assert!(flags.sep24);
    assert!(flags.sep31);
    assert!(flags.sep38);
}

/// Cross-check: every SEP number in `supported_seps` must have its matching
/// flag set to `true` in `supported_sep_feature_flags`, and every flag that is
/// `true` must have its constant present in `supported_seps`.
///
/// This test fails if the two representations drift — for example if a new SEP
/// constant is added to one method but not the other.
#[test]
fn sep_list_and_feature_flags_are_consistent() {
    let env = Env::default();
    let seps = AnchorKitContract::supported_seps(env.clone());
    let flags: SepFeatureFlags = AnchorKitContract::supported_sep_feature_flags(env);

    // For each known SEP, the list and the flag must agree.
    // If `supported_seps` contains the constant then the flag must be true,
    // and if the flag is true then the constant must be in the list.
    let pairs: &[(u32, bool)] = &[
        (SEP_6,  flags.sep6),
        (SEP_10, flags.sep10),
        (SEP_24, flags.sep24),
        (SEP_31, flags.sep31),
        (SEP_38, flags.sep38),
    ];

    for &(sep, flag) in pairs {
        let in_list = seps.contains(&sep);
        assert_eq!(
            in_list, flag,
            "SEP {} is {}in supported_seps but its feature flag is {}; \
             the two representations must agree",
            sep,
            if in_list { "" } else { "not " },
            if flag { "true" } else { "false" },
        );
    }

    // The total count of `true` flags must equal the length of supported_seps
    // so that a flag added without a matching constant (or vice-versa) is caught.
    let enabled_flag_count = [flags.sep6, flags.sep10, flags.sep24, flags.sep31, flags.sep38]
        .iter()
        .filter(|&&f| f)
        .count() as u32;

    assert_eq!(
        seps.len(),
        enabled_flag_count,
        "supported_seps has {} entries but {} feature flags are true; \
         add both the constant and the flag when introducing a new SEP",
        seps.len(),
        enabled_flag_count,
    );
}
