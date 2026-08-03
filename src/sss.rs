use crate::error::Error;
use k256::elliptic_curve::ff::PrimeField;
use k256::{NonZeroScalar, Scalar, SecretKey};
use rand::rngs::OsRng;
use vsss_rs::shamir;
use vsss_rs::{IdentifierPrimeField, ReadableShareSet, ValuePrimeField};

/// A Shamir share: an x-coordinate (identifier) and a y-coordinate (value),
/// both elements of the secp256k1 scalar field.
pub type Share = vsss_rs::DefaultShare<IdentifierPrimeField<Scalar>, ValuePrimeField<Scalar>>;

/// Split a private key into `limit` shares, any `threshold` of which can
/// reconstruct the original key via Lagrange interpolation.
pub fn split(secret: &SecretKey, threshold: usize, limit: usize) -> Result<Vec<Share>, Error> {
    if threshold == 0 {
        return Err(Error::InvalidParams(
            "threshold must be at least 1".to_string(),
        ));
    }
    if threshold > limit {
        return Err(Error::InvalidParams(format!(
            "threshold ({threshold}) cannot exceed share count ({limit})"
        )));
    }
    if limit > u8::MAX as usize {
        return Err(Error::InvalidParams(format!(
            "share count ({limit}) exceeds the supported maximum of 255"
        )));
    }

    let secret = IdentifierPrimeField(*secret.to_nonzero_scalar().as_ref());
    shamir::split_secret::<Share>(threshold, limit, &secret, &mut OsRng)
        .map_err(|e| Error::Vsss(format!("could not split secret: {e}")))
}

/// Reconstruct a private key from a set of shares via Lagrange interpolation.
///
/// The caller must supply at least the threshold number of shares; supplying
/// more than the threshold is safe (the extra points all lie on the same
/// polynomial) but fewer will silently produce a wrong key.
pub fn combine(shares: &[Share]) -> Result<SecretKey, Error> {
    if shares.len() < 2 {
        return Err(Error::NotEnoughShares(2, shares.len()));
    }
    let recovered = shares
        .to_vec()
        .combine()
        .map_err(|e| Error::Vsss(format!("could not combine shares: {e}")))?;
    let scalar: &Scalar = recovered.as_ref();
    let ct: Option<NonZeroScalar> = NonZeroScalar::from_repr(scalar.to_repr()).into();
    let nz = ct.ok_or_else(|| Error::Vsss("reconstructed key is zero".to_string()))?;
    Ok(SecretKey::from(nz))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn random_key() -> SecretKey {
        SecretKey::random(&mut OsRng)
    }

    #[test]
    fn split_combine_round_trip_2_of_3() {
        let key = random_key();
        let shares = split(&key, 2, 3).expect("split");
        assert_eq!(shares.len(), 3);

        for combo in [
            &[shares[0], shares[1]],
            &[shares[0], shares[2]],
            &[shares[1], shares[2]],
        ] {
            let recovered = combine(combo).expect("combine");
            assert_eq!(recovered.to_bytes(), key.to_bytes());
        }
    }

    #[test]
    fn split_combine_round_trip_3_of_5() {
        let key = random_key();
        let shares = split(&key, 3, 5).expect("split");
        let recovered = combine(&shares[..3]).expect("combine");
        assert_eq!(recovered.to_bytes(), key.to_bytes());

        let recovered = combine(&shares[..5]).expect("combine");
        assert_eq!(recovered.to_bytes(), key.to_bytes());
    }

    #[test]
    fn too_few_shares_fails() {
        let key = random_key();
        let shares = split(&key, 2, 3).expect("split");
        assert!(matches!(
            combine(&shares[..1]),
            Err(Error::NotEnoughShares(..))
        ));
    }

    #[test]
    fn invalid_params_rejected() {
        let key = random_key();
        assert!(matches!(split(&key, 0, 3), Err(Error::InvalidParams(_))));
        assert!(matches!(split(&key, 3, 2), Err(Error::InvalidParams(_))));
        assert!(matches!(split(&key, 2, 256), Err(Error::InvalidParams(_))));
    }

    #[test]
    fn duplicate_share_rejected() {
        let key = random_key();
        let shares = split(&key, 2, 3).expect("split");
        let dup = [shares[0], shares[0]];
        assert!(combine(&dup).is_err());
    }
}
