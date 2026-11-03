/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

//! The CG framework of [draft-irtf-cfrg-hybrid-kems-12], which combines a post-quantum KEM with a
//! nominal group, instantiated with ML-KEM and the NIST curves as specified in
//! [draft-irtf-cfrg-concrete-hybrid-kems-04].
//!
//! The MLKEM768-X25519 instance is identical to X-Wing, and is provided by the `x_wing` crate.
//!
//! [draft-irtf-cfrg-hybrid-kems-12]: https://datatracker.ietf.org/doc/html/draft-irtf-cfrg-hybrid-kems-12
//! [draft-irtf-cfrg-concrete-hybrid-kems-04]: https://datatracker.ietf.org/doc/html/draft-irtf-cfrg-concrete-hybrid-kems-04

use cshake::CShake256;
use cshake::digest::{ExtendableOutput, Update};
use elliptic_curve::array::typenum::Unsigned;
use elliptic_curve::sec1::{FromSec1Point, ModulusSize, ToSec1Point};
use elliptic_curve::{Curve, CurveArithmetic, PublicKey, SecretKey};
use ml_kem::kem::{DecapsulationKey, Decapsulator, EncapsulationKey};
use ml_kem::{Decapsulate, Encapsulate, Kem, KeyExport, KeyInit, KeySizeUser, TryKeyInit};
use sha3::{Digest, Sha3_256};
use zeroize::{ZeroizeOnDrop, Zeroizing};

/// Nseed of the hybrid KEM instances, in bytes.
pub(crate) const SEED_SIZE: usize = 32;

/// Nseed of ML-KEM, in bytes.
const PQ_SEED_SIZE: usize = 64;

/// The components of a hybrid KEM instance built with the CG framework.
pub(crate) trait CgParameters {
    /// KEM_PQ
    type PqKem: Kem<DecapsulationKey: KeyInit + Decapsulate>;

    /// Group_T
    type Group: CurveArithmetic<AffinePoint: FromSec1Point<Self::Group> + ToSec1Point<Self::Group>>
        + Curve<FieldBytesSize: ModulusSize>;

    /// Group_T.Nseed
    const GROUP_SEED_SIZE: usize;

    /// The label used for domain separation in the combiner function.
    const LABEL: &'static [u8];
}

/// MLKEM768-P256, as specified in Section 4.1 of [draft-irtf-cfrg-concrete-hybrid-kems-04].
pub(crate) struct MlKem768P256;

impl CgParameters for MlKem768P256 {
    type PqKem = ml_kem::MlKem768;
    type Group = p256::NistP256;
    const GROUP_SEED_SIZE: usize = 128;
    const LABEL: &'static [u8] = b"MLKEM768-P256";
}

/// MLKEM1024-P384, as specified in Section 4.3 of [draft-irtf-cfrg-concrete-hybrid-kems-04].
pub(crate) struct MlKem1024P384;

impl CgParameters for MlKem1024P384 {
    type PqKem = ml_kem::MlKem1024;
    type Group = p384::NistP384;
    const GROUP_SEED_SIZE: usize = 48;
    const LABEL: &'static [u8] = b"MLKEM1024-P384";
}

/// The error returned when a hybrid KEM function fails.
#[derive(Debug)]
pub(crate) struct CgError;

/// A hybrid KEM encapsulation key, the concatenation of ek_PQ and ek_T.
pub(crate) struct CgEncapsulationKey<P: CgParameters> {
    ek_pq: EncapsulationKey<P::PqKem>,
    ek_t: PublicKey<P::Group>,
}

impl<P: CgParameters> CgEncapsulationKey<P> {
    /// KEM_PQ.Nek
    const PQ_SIZE: usize = <EncapsulationKey<P::PqKem> as KeySizeUser>::KeySize::USIZE;

    /// Nek, in bytes.
    pub(crate) const SIZE: usize = Self::PQ_SIZE + group_element_size::<P>();

    /// Parse an encapsulation key, validating both of its components.
    pub(crate) fn from_bytes(bytes: &[u8]) -> Result<Self, CgError> {
        if bytes.len() != Self::SIZE {
            return Err(CgError);
        }
        let (ek_pq, ek_t) = bytes.split_at(Self::PQ_SIZE);
        Ok(Self {
            ek_pq: TryKeyInit::new_from_slice(ek_pq).map_err(|_| CgError)?,
            ek_t: PublicKey::from_sec1_bytes(ek_t).map_err(|_| CgError)?,
        })
    }

    /// Serialize the encapsulation key, as concat(ek_PQ, ek_T).
    pub(crate) fn to_bytes(&self) -> Vec<u8> {
        let mut bytes = self.ek_pq.to_bytes().to_vec();
        bytes.extend_from_slice(self.ek_t.to_sec1_point(false).as_bytes());
        bytes
    }

    /// The Encaps function of the CG framework. Returns the shared secret and the ciphertext.
    pub(crate) fn encapsulate(&self) -> Result<(Vec<u8>, Vec<u8>), CgError> {
        // (ss_PQ, ct_PQ) = KEM_PQ.Encaps(ek_PQ)
        let (ct_pq, ss_pq) = self.ek_pq.encapsulate();

        // sk_E = Group_T.RandomScalar(random(Group_T.Nseed))
        let mut seed = Zeroizing::new(vec![0; P::GROUP_SEED_SIZE]);
        rand::fill(seed.as_mut_slice());
        let sk_e = random_scalar::<P>(&seed)?;

        // ct_T = Group_T.Exp(Group_T.g, sk_E)
        let ct_t = sk_e.public_key().to_sec1_point(false);

        // ss_T = Group_T.ElementToSharedSecret(Group_T.Exp(ek_T, sk_E))
        let ss_t = sk_e.diffie_hellman(&self.ek_t);

        // ss_H = C2PRICombiner(ss_PQ, ss_T, ct_T, ek_T, Label)
        let ss_h = combiner::<P>(
            &ss_pq,
            ss_t.raw_secret_bytes(),
            ct_t.as_bytes(),
            self.ek_t.to_sec1_point(false).as_bytes(),
        );

        // ct_H = concat(ct_PQ, ct_T)
        let mut ct_h = ct_pq.to_vec();
        ct_h.extend_from_slice(ct_t.as_bytes());

        Ok((ss_h, ct_h))
    }
}

impl<P: CgParameters> Clone for CgEncapsulationKey<P> {
    fn clone(&self) -> Self {
        Self {
            ek_pq: self.ek_pq.clone(),
            ek_t: self.ek_t,
        }
    }
}

impl<P: CgParameters> PartialEq for CgEncapsulationKey<P> {
    fn eq(&self, other: &Self) -> bool {
        self.ek_pq == other.ek_pq && self.ek_t == other.ek_t
    }
}

/// A hybrid KEM decapsulation key. It is represented by its seed, and caches the keys expanded from
/// that seed.
pub(crate) struct CgDecapsulationKey<P: CgParameters> {
    seed: Zeroizing<[u8; SEED_SIZE]>,
    dk_pq: DecapsulationKey<P::PqKem>,
    dk_t: SecretKey<P::Group>,
    ek: CgEncapsulationKey<P>,
}

impl<P: CgParameters> CgDecapsulationKey<P> {
    /// The GenerateKeyPair function of the hybrid KEM framework.
    pub(crate) fn generate() -> Result<Self, CgError> {
        // seed = random(Nseed)
        let mut seed = Zeroizing::new([0; SEED_SIZE]);
        rand::fill(seed.as_mut_slice());
        Self::from_seed(&seed)
    }

    /// The DeriveKeyPair function of the CG framework, which is the expandDecapsKeyG function.
    pub(crate) fn from_seed(seed: &[u8; SEED_SIZE]) -> Result<Self, CgError> {
        // seed_full = PRG(seed)
        // NOTE: The PRG is SHAKE256, which is cSHAKE256 with an empty function name and an empty
        // customization string (Section 3.3 of NIST SP 800-185).
        let mut prg = CShake256::default();
        prg.update(seed);
        let mut seed_full = Zeroizing::new(vec![0; PQ_SEED_SIZE + P::GROUP_SEED_SIZE]);
        prg.finalize_xof_into(&mut seed_full);

        // (seed_PQ, seed_T) = split(KEM_PQ.Nseed, Group_T.Nseed, seed_full)
        let (seed_pq, seed_t) = seed_full.split_at(PQ_SEED_SIZE);

        // (dk_PQ, ek_PQ) = KEM_PQ.DeriveKeyPair(seed_PQ)
        let dk_pq = DecapsulationKey::<P::PqKem>::new_from_slice(seed_pq).map_err(|_| CgError)?;
        let ek_pq = dk_pq.encapsulation_key().clone();

        // dk_T = Group_T.RandomScalar(seed_T)
        // ek_T = Group_T.Exp(Group_T.g, dk_T)
        let dk_t = random_scalar::<P>(seed_t)?;
        let ek_t = dk_t.public_key();

        Ok(Self {
            seed: Zeroizing::new(*seed),
            dk_pq,
            dk_t,
            ek: CgEncapsulationKey { ek_pq, ek_t },
        })
    }

    /// The seed this decapsulation key is derived from.
    pub(crate) fn seed(&self) -> &[u8; SEED_SIZE] {
        &self.seed
    }

    /// The encapsulation key corresponding to this decapsulation key.
    pub(crate) fn encapsulation_key(&self) -> &CgEncapsulationKey<P> {
        &self.ek
    }

    /// The Decaps function of the CG framework. Returns the shared secret.
    pub(crate) fn decapsulate(&self, ct: &[u8]) -> Result<Vec<u8>, CgError> {
        // (ct_PQ, ct_T) = split(KEM_PQ.Nct, Group_T.Nelem, ct)
        let pq_size = <P::PqKem as Kem>::CiphertextSize::USIZE;
        if ct.len() != pq_size + group_element_size::<P>() {
            return Err(CgError);
        }
        let (ct_pq, ct_t) = ct.split_at(pq_size);

        // ss_PQ = KEM_PQ.Decaps(dk_PQ, ct_PQ)
        let ss_pq = self.dk_pq.decapsulate_slice(ct_pq).map_err(|_| CgError)?;

        // ss_T = Group_T.ElementToSharedSecret(Group_T.Exp(ct_T, dk_T))
        let ct_t_point = PublicKey::<P::Group>::from_sec1_bytes(ct_t).map_err(|_| CgError)?;
        let ss_t = self.dk_t.diffie_hellman(&ct_t_point);

        // ss_H = C2PRICombiner(ss_PQ, ss_T, ct_T, ek_T, Label)
        Ok(combiner::<P>(
            &ss_pq,
            ss_t.raw_secret_bytes(),
            ct_t,
            self.ek.ek_t.to_sec1_point(false).as_bytes(),
        ))
    }
}

// All the fields of `CgDecapsulationKey` zeroize their secrets on drop.
impl<P: CgParameters> ZeroizeOnDrop for CgDecapsulationKey<P> {}

/// Group_T.Nelem: the size of an uncompressed SEC1 encoded curve point.
const fn group_element_size<P: CgParameters>() -> usize {
    1 + 2 * <<P::Group as Curve>::FieldBytesSize as Unsigned>::USIZE
}

/// The RandomScalar function of the P-256 and P-384 nominal groups, in Section 3.1.1 of
/// [draft-irtf-cfrg-concrete-hybrid-kems-04]: rejection sampling of Nscalar-byte big-endian
/// integers from the seed.
fn random_scalar<P: CgParameters>(seed: &[u8]) -> Result<SecretKey<P::Group>, CgError> {
    let scalar_size = <<P::Group as Curve>::FieldBytesSize as Unsigned>::USIZE;
    seed.chunks_exact(scalar_size)
        .find_map(|candidate| SecretKey::from_slice(candidate).ok())
        .ok_or(CgError)
}

/// The C2PRICombiner function, with SHA3-256 as the KDF.
fn combiner<P: CgParameters>(ss_pq: &[u8], ss_t: &[u8], ct_t: &[u8], ek_t: &[u8]) -> Vec<u8> {
    Sha3_256::new()
        .chain_update(ss_pq)
        .chain_update(ss_t)
        .chain_update(ct_t)
        .chain_update(ek_t)
        .chain_update(P::LABEL)
        .finalize()
        .to_vec()
}
