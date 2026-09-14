use std::{collections::HashSet, fmt, net::IpAddr};

use thiserror::Error;

use crate::config::Strategy;

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) struct DomainName(String);

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct TargetRule {
    host: DomainName,
    include_subdomains: bool,
    strategy: Strategy,
}

#[allow(dead_code)]
#[derive(Debug)]
pub(crate) struct TargetMatcher {
    rules: Vec<TargetRule>,
}

#[derive(Debug, Error, Eq, PartialEq)]
pub(crate) enum DomainError {
    #[error("domain must not be empty")]
    Empty,
    #[error("domain is not valid IDNA: {input}")]
    InvalidIdna { input: String },
    #[error("domain exceeds DNS length limits: {input}")]
    InvalidLength { input: String },
    #[error("IP literals are not valid target domains: {input}")]
    IpLiteral { input: String },
    #[error("duplicate target domain: {host}")]
    Duplicate { host: DomainName },
}

// Matching accessors are consumed by later protocol and proxy groups.
#[allow(dead_code)]
impl DomainName {
    pub(crate) fn parse(input: &str) -> Result<Self, DomainError> {
        if input.is_empty() {
            return Err(DomainError::Empty);
        }
        let without_trailing_dot = input.strip_suffix('.').unwrap_or(input);
        if without_trailing_dot.is_empty() {
            return Err(DomainError::Empty);
        }
        if without_trailing_dot.parse::<IpAddr>().is_ok() {
            return Err(DomainError::IpLiteral {
                input: input.to_owned(),
            });
        }

        let ascii = idna::domain_to_ascii_strict(without_trailing_dot)
            .map_err(|_| DomainError::InvalidIdna {
                input: input.to_owned(),
            })?
            .to_ascii_lowercase();
        if ascii.len() > 253
            || ascii.split('.').any(|label| {
                label.is_empty()
                    || label.len() > 63
                    || label.starts_with('-')
                    || label.ends_with('-')
            })
        {
            return Err(DomainError::InvalidLength {
                input: input.to_owned(),
            });
        }
        if ascii.parse::<IpAddr>().is_ok() {
            return Err(DomainError::IpLiteral {
                input: input.to_owned(),
            });
        }

        Ok(Self(ascii))
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }

    fn label_count(&self) -> usize {
        self.0.bytes().filter(|byte| *byte == b'.').count() + 1
    }

    fn is_subdomain_of(&self, parent: &Self) -> bool {
        self.0.len() > parent.0.len()
            && self.0.ends_with(parent.as_str())
            && self.0.as_bytes()[self.0.len() - parent.0.len() - 1] == b'.'
    }
}

impl fmt::Display for DomainName {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[allow(dead_code)]
impl TargetRule {
    pub(crate) fn new(host: DomainName, include_subdomains: bool, strategy: Strategy) -> Self {
        Self {
            host,
            include_subdomains,
            strategy,
        }
    }

    pub(crate) fn host(&self) -> &DomainName {
        &self.host
    }

    pub(crate) fn includes_subdomains(&self) -> bool {
        self.include_subdomains
    }

    pub(crate) fn strategy(&self) -> Strategy {
        self.strategy
    }
}

#[allow(dead_code)]
impl TargetMatcher {
    pub(crate) fn new(mut rules: Vec<TargetRule>) -> Result<Self, DomainError> {
        let mut seen = HashSet::with_capacity(rules.len());
        for rule in &rules {
            if !seen.insert(rule.host.clone()) {
                return Err(DomainError::Duplicate {
                    host: rule.host.clone(),
                });
            }
        }
        rules.sort_by_key(|rule| std::cmp::Reverse(rule.host.label_count()));
        Ok(Self { rules })
    }

    pub(crate) fn find(&self, host: &DomainName) -> Option<&TargetRule> {
        self.rules.iter().find(|rule| {
            host == rule.host() || (rule.includes_subdomains() && host.is_subdomain_of(rule.host()))
        })
    }

    pub(crate) fn rules(&self) -> &[TargetRule] {
        &self.rules
    }

    pub(crate) fn into_rules(self) -> Vec<TargetRule> {
        self.rules
    }
}

#[cfg(test)]
mod tests {
    use crate::config::Strategy;

    use super::{DomainError, DomainName, TargetMatcher, TargetRule};

    fn rule(host: &str, include_subdomains: bool) -> TargetRule {
        TargetRule::new(
            DomainName::parse(host).expect("valid test hostname"),
            include_subdomains,
            Strategy::TlsRecordSplit,
        )
    }

    #[test]
    fn canonicalizes_case_trailing_dot_and_idna() {
        let mixed = DomainName::parse("BÜCHER.Example.").expect("valid IDNA hostname");
        let ascii = DomainName::parse("xn--bcher-kva.example").expect("valid ASCII hostname");

        assert_eq!(mixed, ascii);
        assert_eq!(mixed.as_str(), "xn--bcher-kva.example");
    }

    #[test]
    fn matches_exact_and_enabled_subdomains_without_suffix_overmatch() {
        let matcher = TargetMatcher::new(vec![rule("reddit.com", true)]).expect("valid matcher");

        assert!(
            matcher
                .find(&DomainName::parse("reddit.com").unwrap())
                .is_some()
        );
        assert!(
            matcher
                .find(&DomainName::parse("www.reddit.com").unwrap())
                .is_some()
        );
        assert!(
            matcher
                .find(&DomainName::parse("notreddit.com").unwrap())
                .is_none()
        );
        assert!(
            matcher
                .find(&DomainName::parse("reddit.com.example").unwrap())
                .is_none()
        );
    }

    #[test]
    fn exact_rule_wins_over_parent_subdomain_rule() {
        let matcher = TargetMatcher::new(vec![
            rule("example.com", true),
            rule("api.example.com", false),
        ])
        .expect("valid matcher");

        let matched = matcher
            .find(&DomainName::parse("api.example.com").unwrap())
            .expect("exact rule");
        assert_eq!(matched.host().as_str(), "api.example.com");
        assert_eq!(
            matcher
                .find(&DomainName::parse("x.api.example.com").unwrap())
                .unwrap()
                .host()
                .as_str(),
            "example.com"
        );
    }

    #[test]
    fn rejects_invalid_domains_and_canonical_duplicates() {
        for invalid in ["", ".", "example..com", "127.0.0.1", "::1", "-bad.example"] {
            assert!(DomainName::parse(invalid).is_err(), "accepted {invalid}");
        }

        let duplicate =
            TargetMatcher::new(vec![rule("Reddit.com", true), rule("reddit.com.", false)]);
        assert!(matches!(duplicate, Err(DomainError::Duplicate { .. })));
    }

    #[test]
    fn exact_only_rule_does_not_match_subdomains() {
        let matcher = TargetMatcher::new(vec![rule("medium.com", false)]).expect("valid matcher");
        assert!(
            matcher
                .find(&DomainName::parse("medium.com").unwrap())
                .is_some()
        );
        assert!(
            matcher
                .find(&DomainName::parse("www.medium.com").unwrap())
                .is_none()
        );
        assert_eq!(matcher.rules().len(), 1);
    }
}
