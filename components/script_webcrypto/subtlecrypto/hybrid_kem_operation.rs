/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/. */

use js::context::JSContext;
use script_bindings::codegen::GenericBindings::CryptoKeyBinding::{
    CryptoKeyMethods, CryptoKeyPair, KeyType, KeyUsage,
};
use script_bindings::codegen::GenericBindings::SubtleCryptoBinding::{JsonWebKey, KeyFormat};
use script_bindings::error::Error;
use script_bindings::root::DomRoot;
use script_bindings::str::DOMString;
use x_wing::{Decapsulate, Decapsulator, Encapsulate, Generate, KeyExport, TryKeyInit};

use crate::cryptokey::{CryptoKey, Handle, KeyUsageSliceHelper};
use crate::subtlecrypto::hybrid_kem_cg::{
    self, CgDecapsulationKey, CgEncapsulationKey, MlKem768P256, MlKem1024P384,
};
use crate::subtlecrypto::{
    Algorithm, CryptoAlgorithm, EncapsulatedBits, ExportedKey, JsonWebKeyExt, JwkStringField,
    KeyAlgorithm, KeyAlgorithmAndDerivatives,
};
use crate::traits::Equivalence;

/// <https://wicg.github.io/webcrypto-modern-algos/#hybrid-kems-operations-encapsulate>
pub(crate) fn encapsulate<D: Equivalence>(key: &CryptoKey<D>) -> Result<EncapsulatedBits, Error> {
    // Step 1. If the [[type]] internal slot of key is not "public", then throw an
    // InvalidAccessError.
    key.ensure_type(KeyType::Public)?;

    // Step 2. Let sharedKey and ciphertext be the outputs that result from performing the Encaps
    // function for the hybrid KEM instance indicated by the name member of algorithm in Section 4
    // of [draft-irtf-cfrg-concrete-hybrid-kems-04], using the key represented by the [[handle]]
    // internal slot of key as the ek input parameter.
    // Step 3. If the Encaps function returned an error, return an OperationError.
    let (shared_key, ciphertext) = match key.handle() {
        Handle::MlKem768P256PublicKey(public_key) => public_key.encapsulate(),
        Handle::MlKem768X25519PublicKey(public_key) => {
            let (ciphertext, shared_key) = public_key.encapsulate();
            Ok((shared_key.to_vec(), ciphertext.to_vec()))
        },
        Handle::MlKem1024P384PublicKey(public_key) => public_key.encapsulate(),
        _ => {
            return Err(Error::Operation(Some(
                "The key handle is not representing a hybrid KEM public key".into(),
            )));
        },
    }
    .map_err(|_| Error::Operation(Some("Failed to perform hybrid KEM encapsulation".into())))?;

    // Step 4. Let result be a new EncapsulatedBits dictionary.
    // Step 5. Set the sharedKey attribute of result to the result of creating an ArrayBuffer
    // containing sharedKey.
    // Step 6. Set the ciphertext attribute of result to the result of creating an ArrayBuffer
    // containing ciphertext.
    let result = EncapsulatedBits {
        shared_key: Some(shared_key.into()),
        ciphertext: Some(ciphertext),
    };

    // Step 7. Return result.
    Ok(result)
}

/// <https://wicg.github.io/webcrypto-modern-algos/#hybrid-kems-operations-decapsulate>
pub(crate) fn decapsulate<D: Equivalence>(
    key: &CryptoKey<D>,
    ciphertext: &[u8],
) -> Result<Vec<u8>, Error> {
    // Step 1. If the [[type]] internal slot of key is not "private", then throw an
    // InvalidAccessError.
    key.ensure_type(KeyType::Private)?;

    // Step 2. Let sharedKey be the output that results from performing the Decaps function for the
    // hybrid KEM instance indicated by the name member of algorithm in Section 4 of
    // [draft-irtf-cfrg-concrete-hybrid-kems-04], using the key represented by the [[handle]]
    // internal slot of key as the dk input parameter, and ciphertext as the ct input parameter.
    // Step 3. If the Decaps function returned an error, return an OperationError.
    let shared_key = match key.handle() {
        Handle::MlKem768P256PrivateKey(private_key) => private_key.decapsulate(ciphertext).ok(),
        Handle::MlKem768X25519PrivateKey(private_key) => private_key
            .decapsulate_slice(ciphertext)
            .ok()
            .map(|shared_key| shared_key.to_vec()),
        Handle::MlKem1024P384PrivateKey(private_key) => private_key.decapsulate(ciphertext).ok(),
        _ => {
            return Err(Error::Operation(Some(
                "The key handle is not representing a hybrid KEM private key".into(),
            )));
        },
    }
    .ok_or_else(|| Error::Operation(Some("Failed to perform hybrid KEM decapsulation".into())))?;

    // Step 4. Return sharedKey.
    Ok(shared_key)
}

/// <https://wicg.github.io/webcrypto-modern-algos/#hybrid-kems-operations-get-shared-key-length>
pub(crate) fn get_shared_key_length() -> u32 {
    // Step 1. Return 256.
    256
}

/// <https://wicg.github.io/webcrypto-modern-algos/#hybrid-kems-operations-generate-key>
pub(crate) fn generate_key<D: Equivalence>(
    cx: &mut JSContext,
    global: &D::GlobalScope,
    normalized_algorithm: &Algorithm,
    extractable: bool,
    usages: Vec<KeyUsage>,
) -> Result<CryptoKeyPair<D>, Error> {
    // Step 1. If usages contains an entry which is not one of "encapsulateKey", "encapsulateBits",
    // "decapsulateKey" or "decapsulateBits", then throw a SyntaxError.
    usages.ensure_only_contain_entries_from(&[
        KeyUsage::EncapsulateKey,
        KeyUsage::EncapsulateBits,
        KeyUsage::DecapsulateKey,
        KeyUsage::DecapsulateBits,
    ])?;

    // Step 2. Generate a hybrid KEM key pair by performing the GenerateKeyPair function described
    // in Section 5.2 of [draft-irtf-cfrg-hybrid-kems-12] with the hybrid KEM instance indicated by
    // the name member of normalizedAlgorithm.
    // Step 3. If the key generation step fails, then throw an OperationError.
    let private_key_handle = match normalized_algorithm.name {
        CryptoAlgorithm::MlKem768P256 => {
            CgDecapsulationKey::generate().map(Handle::MlKem768P256PrivateKey)
        },
        CryptoAlgorithm::MlKem768X25519 => Ok(Handle::MlKem768X25519PrivateKey(
            x_wing::DecapsulationKey::generate(),
        )),
        CryptoAlgorithm::MlKem1024P384 => {
            CgDecapsulationKey::generate().map(Handle::MlKem1024P384PrivateKey)
        },
        name => {
            return Err(Error::NotSupported(Some(format!(
                "{} is not a hybrid KEM algorithm",
                name.as_str()
            ))));
        },
    }
    .map_err(|_| Error::Operation(Some("Failed to generate hybrid KEM key pair".into())))?;
    let public_key_handle = corresponding_public_key(&private_key_handle)?;

    // Step 4. Let algorithm be a new KeyAlgorithm object.
    // Step 5. Set the name attribute of algorithm to the name attribute of normalizedAlgorithm.
    let algorithm = KeyAlgorithm {
        name: normalized_algorithm.name,
    };

    // Step 6. Let publicKey be a new CryptoKey representing the encapsulation key of the generated
    // key pair.
    // Step 7. Set the [[type]] internal slot of publicKey to "public".
    // Step 8. Set the [[algorithm]] internal slot of publicKey to algorithm.
    // Step 9. Set the [[extractable]] internal slot of publicKey to true.
    // Step 10. Set the [[usages]] internal slot of publicKey to be the usage intersection of usages
    // and [ "encapsulateKey", "encapsulateBits" ].
    let public_key = CryptoKey::new(
        cx,
        global,
        KeyType::Public,
        true,
        KeyAlgorithmAndDerivatives::KeyAlgorithm(algorithm.clone()),
        usages.usage_intersection(&[KeyUsage::EncapsulateKey, KeyUsage::EncapsulateBits]),
        public_key_handle,
    );

    // Step 11. Let privateKey be a new CryptoKey representing the decapsulation key of the
    // generated key pair.
    // Step 12. Set the [[type]] internal slot of privateKey to "private".
    // Step 13. Set the [[algorithm]] internal slot of privateKey to algorithm.
    // Step 14. Set the [[extractable]] internal slot of privateKey to extractable.
    // Step 15. Set the [[usages]] internal slot of privateKey to be the usage intersection of
    // usages and [ "decapsulateKey", "decapsulateBits" ].
    let private_key = CryptoKey::new(
        cx,
        global,
        KeyType::Private,
        extractable,
        KeyAlgorithmAndDerivatives::KeyAlgorithm(algorithm),
        usages.usage_intersection(&[KeyUsage::DecapsulateKey, KeyUsage::DecapsulateBits]),
        private_key_handle,
    );

    // Step 16. Let result be a new CryptoKeyPair dictionary.
    // Step 17. Set the publicKey attribute of result to be publicKey.
    // Step 18. Set the privateKey attribute of result to be privateKey.
    let result = CryptoKeyPair {
        publicKey: Some(public_key),
        privateKey: Some(private_key),
    };

    // Step 19. Return result.
    Ok(result)
}

/// <https://wicg.github.io/webcrypto-modern-algos/#hybrid-kems-operations-import-key>
pub(crate) fn import_key<D: Equivalence>(
    cx: &mut JSContext,
    global: &D::GlobalScope,
    normalized_algorithm: &Algorithm,
    format: KeyFormat,
    key_data: &[u8],
    extractable: bool,
    usages: Vec<KeyUsage>,
) -> Result<DomRoot<CryptoKey<D>>, Error> {
    // Step 1. Let keyData be the key data to be imported.

    // Step 2.
    let key = match format {
        // If format is "raw-public":
        KeyFormat::Raw_public => {
            // Step 2.1. If usages contains an entry which is not "encapsulateKey" or
            // "encapsulateBits" then throw a SyntaxError.
            usages.ensure_only_contain_entries_from(&[
                KeyUsage::EncapsulateKey,
                KeyUsage::EncapsulateBits,
            ])?;

            // Step 2.2. Let data be keyData.
            let data = key_data;

            // Step 2.3. If the length in bytes of data is not the raw public key length, Nek, for
            // the hybrid KEM instance indicated by the name member of normalizedAlgorithm in
            // Section 4 of [draft-irtf-cfrg-concrete-hybrid-kems-04], then throw a DataError.
            // Step 2.4. Let key be a new CryptoKey that represents the hybrid KEM public key data
            // in data.
            // Step 2.5. Set the [[type]] internal slot of key to "public"
            // Step 2.6. Let algorithm be a new KeyAlgorithm.
            // Step 2.7. Set the name attribute of algorithm to the name attribute of
            // normalizedAlgorithm.
            // Step 2.8. Set the [[algorithm]] internal slot of key to algorithm.
            let public_key = import_public_key(normalized_algorithm.name, data)?;
            let algorithm = KeyAlgorithm {
                name: normalized_algorithm.name,
            };
            CryptoKey::new(
                cx,
                global,
                KeyType::Public,
                extractable,
                KeyAlgorithmAndDerivatives::KeyAlgorithm(algorithm),
                usages.normalized_value(),
                public_key,
            )
        },
        // If format is "raw-seed":
        KeyFormat::Raw_seed => {
            // Step 2.1. If usages contains an entry which is not "decapsulateKey" or
            // "decapsulateBits" then throw a SyntaxError.
            usages.ensure_only_contain_entries_from(&[
                KeyUsage::DecapsulateKey,
                KeyUsage::DecapsulateBits,
            ])?;

            // Step 2.2. Let data be keyData.
            let data = key_data;

            // Step 2.3. If the length in bits of data is not 256 then throw a DataError.
            if data.len() != 32 {
                return Err(Error::Data(Some(
                    "The length in bits of data is not 256".into(),
                )));
            }

            // Step 2.4. Let keyPair be the result of performing the DeriveKeyPair function
            // described in Section 5.5 of [draft-irtf-cfrg-hybrid-kems-12] with the hybrid KEM
            // instance indicated by the name member of normalizedAlgorithm, using data as the seed
            // input parameter.
            // Step 2.5. If the DeriveKeyPair function returned an error, then throw an
            // OperationError.
            let private_key = derive_key_pair(normalized_algorithm.name, data)?;

            // Step 2.6. Let key be a new CryptoKey that represents the hybrid KEM private key
            // identified by the decapsulation key of keyPair.
            // Step 2.7. Set the [[type]] internal slot of key to "private"
            // Step 2.8. Let algorithm be a new KeyAlgorithm.
            // Step 2.9. Set the name attribute of algorithm to the name attribute of
            // normalizedAlgorithm.
            // Step 2.10. Set the [[algorithm]] internal slot of key to algorithm.
            let algorithm = KeyAlgorithm {
                name: normalized_algorithm.name,
            };
            CryptoKey::new(
                cx,
                global,
                KeyType::Private,
                extractable,
                KeyAlgorithmAndDerivatives::KeyAlgorithm(algorithm),
                usages.normalized_value(),
                private_key,
            )
        },
        // If format is "jwk":
        KeyFormat::Jwk => {
            // Step 2.1.
            // If keyData is a JsonWebKey dictionary:
            //     Let jwk equal keyData.
            // Otherwise:
            //     Throw a DataError.
            let jwk = JsonWebKey::parse(cx, key_data)?;

            // Step 2.2. If the priv field of jwk is present and if usages contains an entry which
            // is not "decapsulateKey" or "decapsulateBits" then throw a SyntaxError.
            // Step 2.3. If the priv field of jwk is not present and if usages contains an entry
            // which is not "encapsulateKey" or "encapsulateBits" then throw a SyntaxError.
            match jwk.priv_.as_ref() {
                Some(_) => usages.ensure_only_contain_entries_from(&[
                    KeyUsage::DecapsulateKey,
                    KeyUsage::DecapsulateBits,
                ])?,
                None => usages.ensure_only_contain_entries_from(&[
                    KeyUsage::EncapsulateKey,
                    KeyUsage::EncapsulateBits,
                ])?,
            }

            // Step 2.4. If the kty field of jwk is not "AKP", then throw a DataError.
            if jwk.kty.as_ref().is_none_or(|kty| kty != "AKP") {
                return Err(Error::Data(Some(
                    "The kty field of jwk is not \"AKP\"".into(),
                )));
            }

            // Step 2.5. If the alg field of jwk is not present, or its value does not identify the
            // hybrid KEM instance indicated by the name member of normalizedAlgorithm, then throw a
            // DataError.
            if jwk
                .alg
                .as_ref()
                .is_none_or(|alg| alg != normalized_algorithm.name.as_str())
            {
                return Err(Error::Data(Some("The alg field of jwk is invalid".into())));
            }

            // Step 2.6. If usages is non-empty and the use field of jwk is present and is not equal
            // to "enc", then throw a DataError.
            if !usages.is_empty() && jwk.use_.as_ref().is_some_and(|use_| use_ != "enc") {
                return Err(Error::Data(Some(
                    "Usages is non-empty and the use field of jwk is present and is not \
                        equal to \"enc\""
                        .into(),
                )));
            }

            // Step 2.7. If the key_ops field of jwk is present, and is invalid according to the
            // requirements of JSON Web Key [JWK], or it does not contain all of the specified
            // usages values, then throw a DataError.
            jwk.check_key_ops(&usages)?;

            // Step 2.8. If the ext field of jwk is present and has the value false and extractable
            // is true, then throw a DataError.
            if jwk.ext.is_some_and(|ext| !ext) && extractable {
                return Err(Error::Data(Some(
                    "The ext field of jwk is present and has the value false and extractable \
                        is true"
                        .into(),
                )));
            }

            // Step 2.9.
            // If the priv field of jwk is present:
            let (key_type, key_handle) = if jwk.priv_.is_some() {
                // Step 2.9.1. If the priv attribute of jwk does not contain a valid base64url
                // encoded 32-byte seed representing a hybrid KEM private key, then throw a
                // DataError.
                let priv_bytes = jwk.decode_required_string_field(JwkStringField::Priv)?;
                if priv_bytes.len() != 32 {
                    return Err(Error::Data(Some(
                        "The priv attribute of jwk does not contain a valid base64url \
                            encoded 32-byte seed"
                            .into(),
                    )));
                }

                // Step 2.9.2. Let key be a new CryptoKey object that represents the hybrid KEM
                // private key identified by interpreting the priv attribute of jwk as a base64url
                // encoded seed.
                // Step 2.9.3. Set the [[type]] internal slot of key to "private".
                // Step 2.9.4. If the pub attribute of jwk does not contain the base64url encoded
                // public key representing the hybrid KEM public key corresponding to key, then
                // throw a DataError.
                // NOTE: The CryptoKey object is created in Step 2.10 - 2.12.
                let private_key_handle = derive_key_pair(normalized_algorithm.name, &priv_bytes)?;
                let pub_bytes = jwk.decode_required_string_field(JwkStringField::Pub)?;
                if *pub_bytes != export_public_key(&corresponding_public_key(&private_key_handle)?)?
                {
                    return Err(Error::Data(Some(
                        "The public key in pub attribute does not match \
                            the private key in priv attribute"
                            .into(),
                    )));
                }
                (KeyType::Private, private_key_handle)
            }
            // Otherwise:
            else {
                // Step 2.9.1. If the pub attribute of jwk does not contain a valid base64url
                // encoded raw public key whose length is Nek for the hybrid KEM instance indicated
                // by the name member of normalizedAlgorithm in Section 4 of
                // [draft-irtf-cfrg-concrete-hybrid-kems-04], then throw a DataError.
                // Step 2.9.2. Let key be a new CryptoKey object that represents the hybrid KEM
                // public key identified by interpreting the pub attribute of jwk as a base64url
                // encoded public key.
                // Step 2.9.3. Set the [[type]] internal slot of key to "public".
                // NOTE: The CryptoKey object is created in Step 2.10 - 2.12.
                let pub_bytes = jwk.decode_required_string_field(JwkStringField::Pub)?;
                let public_key_handle = import_public_key(normalized_algorithm.name, &pub_bytes)?;
                (KeyType::Public, public_key_handle)
            };

            // Step 2.10. Let algorithm be a new instance of a KeyAlgorithm object.
            // Step 2.11. Set the name attribute of algorithm to the name member of
            // normalizedAlgorithm.
            // Step 2.12. Set the [[algorithm]] internal slot of key to algorithm.
            let algorithm = KeyAlgorithm {
                name: normalized_algorithm.name,
            };
            CryptoKey::new(
                cx,
                global,
                key_type,
                extractable,
                KeyAlgorithmAndDerivatives::KeyAlgorithm(algorithm),
                usages.normalized_value(),
                key_handle,
            )
        },
        // Otherwise:
        _ => {
            // throw a NotSupportedError.
            return Err(Error::NotSupported(Some(
                "Unsupported import key format for hybrid KEM key".into(),
            )));
        },
    };

    // Step 3. Return key.
    Ok(key)
}

/// <https://wicg.github.io/webcrypto-modern-algos/#hybrid-kems-operations-export-key>
pub(crate) fn export_key<D: Equivalence>(
    format: KeyFormat,
    key: &CryptoKey<D>,
) -> Result<ExportedKey, Error> {
    // Step 1. Let key be the CryptoKey to be exported.

    // Step 2. If the underlying cryptographic key material represented by the [[handle]] internal
    // slot of key cannot be accessed, then throw an OperationError.
    // NOTE: Done in Step 3.

    // Step 3.
    let result = match format {
        // If format is "raw-public":
        KeyFormat::Raw_public => {
            // Step 3.1. If the [[type]] internal slot of key is not "public", then throw an
            // InvalidAccessError.
            key.ensure_type(KeyType::Public)?;

            // Step 3.2. Let data be a byte sequence containing the raw octets of the key
            // represented by the [[handle]] internal slot of key.
            let data = export_public_key(key.handle())?;

            // Step 3.3. Let result be data.
            ExportedKey::new_bytes(data)
        },
        // If format is "raw-seed":
        KeyFormat::Raw_seed => {
            // Step 3.1. If the [[type]] internal slot of key is not "private", then throw an
            // InvalidAccessError.
            key.ensure_type(KeyType::Private)?;

            // Step 3.2. Let data be a byte sequence containing the 32-byte seed represented by the
            // [[handle]] internal slot of key.
            let data = export_seed(key.handle())?;

            // Step 3.3. Let result be data.
            ExportedKey::new_bytes(data)
        },
        // If format is "jwk":
        KeyFormat::Jwk => {
            // Step 3.1. Let jwk be a new JsonWebKey dictionary.
            let mut jwk = JsonWebKey::default();

            // Step 3.2. Let keyAlgorithm be the [[algorithm]] internal slot of key.
            let KeyAlgorithmAndDerivatives::KeyAlgorithm(key_algorithm) = key.algorithm() else {
                return Err(Error::Operation(Some(
                    "[[algorithm]] internal slot of key is not a KeyAlgorithm".into(),
                )));
            };

            // Step 3.3. Set the kty attribute of jwk to "AKP".
            jwk.kty = Some(DOMString::from_static("AKP"));

            // Step 3.4. Set the alg attribute of jwk to the name member of keyAlgorithm.
            jwk.alg = Some(DOMString::from(key_algorithm.name.as_str()));

            // Step 3.5. Set the pub attribute of jwk to the base64url encoded public key
            // corresponding to the [[handle]] internal slot of key.
            // Step 3.6.
            // If the [[type]] internal slot of key is "private":
            //     Set the priv attribute of jwk to the base64url encoded 32-byte seed represented
            //     by the [[handle]] internal slot of key.
            if key.Type() == KeyType::Private {
                jwk.encode_string_field(
                    JwkStringField::Pub,
                    &export_public_key(&corresponding_public_key(key.handle())?)?,
                );
                jwk.encode_string_field(JwkStringField::Priv, &export_seed(key.handle())?);
            } else {
                jwk.encode_string_field(JwkStringField::Pub, &export_public_key(key.handle())?);
            }

            // Step 3.7. Set the key_ops attribute of jwk to the usages attribute of key.
            jwk.set_key_ops(key.usages());

            // Step 3.8. Set the ext attribute of jwk to the [[extractable]] internal slot of key.
            jwk.ext = Some(key.Extractable());

            // Step 3.9. Let result be jwk.
            ExportedKey::new_jwk(jwk)
        },
        // Otherwise:
        _ => {
            // throw a NotSupportedError.
            return Err(Error::NotSupported(Some(
                "Unsupported export key format for hybrid KEM key".into(),
            )));
        },
    };

    // Step 4.  Return result.
    Ok(result)
}

/// <https://wicg.github.io/webcrypto-modern-algos/#SubtleCrypto-method-getPublicKey>
/// Step 9 - 15, for hybrid KEM
pub(crate) fn get_public_key<D: Equivalence>(
    cx: &mut JSContext,
    global: &D::GlobalScope,
    key: &CryptoKey<D>,
    algorithm: &KeyAlgorithmAndDerivatives,
    usages: Vec<KeyUsage>,
) -> Result<DomRoot<CryptoKey<D>>, Error> {
    // Step 9. If usages contains an entry which is not supported for a public key by the algorithm
    // identified by algorithm, then throw a SyntaxError.
    //
    // NOTE: See "importKey" operation for supported usages
    usages
        .ensure_only_contain_entries_from(&[KeyUsage::EncapsulateKey, KeyUsage::EncapsulateBits])?;

    // Step 10. Let publicKey be a new CryptoKey representing the public key corresponding to the
    // private key represented by the [[handle]] internal slot of key.
    // Step 11. If an error occurred, then throw a OperationError.
    // Step 12. Set the [[type]] internal slot of publicKey to "public".
    // Step 13. Set the [[algorithm]] internal slot of publicKey to algorithm.
    // Step 14. Set the [[extractable]] internal slot of publicKey to true.
    // Step 15. Set the [[usages]] internal slot of publicKey to usages.
    let public_key_handle = corresponding_public_key(key.handle())?;
    let public_key = CryptoKey::new(
        cx,
        global,
        KeyType::Public,
        true,
        algorithm.clone(),
        usages,
        public_key_handle,
    );

    Ok(public_key)
}

/// Perform the DeriveKeyPair function described in Section 5.5 of
/// [draft-irtf-cfrg-hybrid-kems-12] with the hybrid KEM instance indicated by `name`, using `seed`
/// as the seed input parameter, and return the handle of the private key.
fn derive_key_pair(name: CryptoAlgorithm, seed: &[u8]) -> Result<Handle, Error> {
    let seed: &[u8; hybrid_kem_cg::SEED_SIZE] = seed
        .try_into()
        .map_err(|_| Error::Data(Some("The length in bits of the seed is not 256".into())))?;
    match name {
        CryptoAlgorithm::MlKem768P256 => {
            CgDecapsulationKey::from_seed(seed).map(Handle::MlKem768P256PrivateKey)
        },
        CryptoAlgorithm::MlKem768X25519 => Ok(Handle::MlKem768X25519PrivateKey(
            x_wing::DecapsulationKey::from(*seed),
        )),
        CryptoAlgorithm::MlKem1024P384 => {
            CgDecapsulationKey::from_seed(seed).map(Handle::MlKem1024P384PrivateKey)
        },
        name => {
            return Err(Error::NotSupported(Some(format!(
                "{} is not a hybrid KEM algorithm",
                name.as_str()
            ))));
        },
    }
    .map_err(|_| Error::Operation(Some("Failed to derive hybrid KEM key pair".into())))
}

/// Create the handle of a hybrid KEM public key, for the hybrid KEM instance indicated by `name`,
/// from its raw octets.
fn import_public_key(name: CryptoAlgorithm, data: &[u8]) -> Result<Handle, Error> {
    // If the length in bytes of data is not the raw public key length, Nek, for the hybrid KEM
    // instance indicated by name in Section 4 of [draft-irtf-cfrg-concrete-hybrid-kems-04], then
    // throw a DataError.
    let encapsulation_key_size = match name {
        CryptoAlgorithm::MlKem768P256 => CgEncapsulationKey::<MlKem768P256>::SIZE,
        CryptoAlgorithm::MlKem768X25519 => x_wing::ENCAPSULATION_KEY_SIZE,
        CryptoAlgorithm::MlKem1024P384 => CgEncapsulationKey::<MlKem1024P384>::SIZE,
        name => {
            return Err(Error::NotSupported(Some(format!(
                "{} is not a hybrid KEM algorithm",
                name.as_str()
            ))));
        },
    };
    if data.len() != encapsulation_key_size {
        return Err(Error::Data(Some(format!(
            "Invalid key length for {} public key",
            name.as_str()
        ))));
    }

    match name {
        CryptoAlgorithm::MlKem768P256 => CgEncapsulationKey::from_bytes(data)
            .map(Handle::MlKem768P256PublicKey)
            .ok(),
        CryptoAlgorithm::MlKem768X25519 => x_wing::EncapsulationKey::new_from_slice(data)
            .map(Handle::MlKem768X25519PublicKey)
            .ok(),
        CryptoAlgorithm::MlKem1024P384 => CgEncapsulationKey::from_bytes(data)
            .map(Handle::MlKem1024P384PublicKey)
            .ok(),
        _ => None,
    }
    .ok_or_else(|| {
        Error::Data(Some(format!(
            "Failed to parse the {} public key",
            name.as_str()
        )))
    })
}

/// Return the handle of the hybrid KEM public key corresponding to the hybrid KEM private key
/// represented by `handle`.
fn corresponding_public_key(handle: &Handle) -> Result<Handle, Error> {
    match handle {
        Handle::MlKem768P256PrivateKey(private_key) => Ok(Handle::MlKem768P256PublicKey(
            private_key.encapsulation_key().clone(),
        )),
        Handle::MlKem768X25519PrivateKey(private_key) => Ok(Handle::MlKem768X25519PublicKey(
            private_key.encapsulation_key().clone(),
        )),
        Handle::MlKem1024P384PrivateKey(private_key) => Ok(Handle::MlKem1024P384PublicKey(
            private_key.encapsulation_key().clone(),
        )),
        _ => Err(Error::Operation(Some(
            "The key handle is not representing a hybrid KEM private key".into(),
        ))),
    }
}

/// Return the raw octets of the hybrid KEM public key represented by `handle`.
fn export_public_key(handle: &Handle) -> Result<Vec<u8>, Error> {
    match handle {
        Handle::MlKem768P256PublicKey(public_key) => Ok(public_key.to_bytes()),
        Handle::MlKem768X25519PublicKey(public_key) => Ok(public_key.to_bytes().to_vec()),
        Handle::MlKem1024P384PublicKey(public_key) => Ok(public_key.to_bytes()),
        _ => Err(Error::Operation(Some(
            "The key handle is not representing a hybrid KEM public key".into(),
        ))),
    }
}

/// Return the 32-byte seed represented by the hybrid KEM private key `handle`.
fn export_seed(handle: &Handle) -> Result<Vec<u8>, Error> {
    match handle {
        Handle::MlKem768P256PrivateKey(private_key) => Ok(private_key.seed().to_vec()),
        Handle::MlKem768X25519PrivateKey(private_key) => Ok(private_key.as_bytes().to_vec()),
        Handle::MlKem1024P384PrivateKey(private_key) => Ok(private_key.seed().to_vec()),
        _ => Err(Error::Operation(Some(
            "The key handle is not representing a hybrid KEM private key".into(),
        ))),
    }
}
