// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

//! Mocks of the traits supplied by the user of the crate..

use dusk_bls12_381_sign::PublicKey;
use dusk_jubjub::{BlsScalar, JubJubAffine, JubJubScalar};
use dusk_pki::{PublicSpendKey, ViewKey};
use dusk_plonk::prelude::Proof;
use dusk_schnorr::Signature;
use dusk_wallet_core::{
    EnrichedNote, ProverClient, StakeInfo, StateClient, Store, Transaction,
    UnprovenTransaction, Wallet, POSEIDON_TREE_DEPTH,
};
use phoenix_core::{Crossover, Fee, Note, NoteType};
use poseidon_merkle::{Item, Opening as PoseidonOpening, Tree};
use rand_core::{CryptoRng, RngCore};

fn default_opening() -> PoseidonOpening<(), POSEIDON_TREE_DEPTH, 4> {
    // Build a "default" opening
    const POS: u64 = 42;
    let mut tree = Tree::new();
    tree.insert(
        POS,
        Item {
            hash: BlsScalar::zero(),
            data: (),
        },
    );
    tree.opening(POS).unwrap()
}

/// Create a new wallet meant for tests. It includes a client that will always
/// return a random anchor (same every time), and the default opening.
///
/// The number of notes available is determined by `note_values`.
pub fn mock_wallet<Rng: RngCore + CryptoRng>(
    rng: &mut Rng,
    note_values: &[u64],
) -> Wallet<TestStore, TestStateClient, TestProverClient> {
    let store = TestStore::new(rng);
    let psk = store.retrieve_ssk(0).unwrap().public_spend_key();

    let notes = new_notes(rng, &psk, note_values);
    let anchor = BlsScalar::random(rng);
    let opening = default_opening();

    let state = TestStateClient::new(notes, anchor, opening);
    let prover = TestProverClient;

    Wallet::new(store, state, prover)
}

/// Create a new wallet equivalent in all ways to `mock_wallet`, but serializing
/// and deserializing a `Transaction` using `rkyv`.
pub fn mock_canon_wallet<Rng: RngCore + CryptoRng>(
    rng: &mut Rng,
    note_values: &[u64],
) -> Wallet<TestStore, TestStateClient, RkyvProverClient> {
    let store = TestStore::new(rng);
    let psk = store.retrieve_ssk(0).unwrap().public_spend_key();

    let notes = new_notes(rng, &psk, note_values);
    let anchor = BlsScalar::random(rng);
    let opening = default_opening();

    let state = TestStateClient::new(notes, anchor, opening);
    let prover = RkyvProverClient {
        prover: TestProverClient,
    };

    Wallet::new(store, state, prover)
}

/// Create a new wallet equivalent in all ways to `mock_wallet`, but serializing
/// and deserializing an `UnprovenTransaction` using `dusk::bytes`.
pub fn mock_serde_wallet<Rng: RngCore + CryptoRng>(
    rng: &mut Rng,
    note_values: &[u64],
) -> Wallet<TestStore, TestStateClient, SerdeProverClient> {
    let store = TestStore::new(rng);
    let psk = store.retrieve_ssk(0).unwrap().public_spend_key();

    let notes = new_notes(rng, &psk, note_values);
    let anchor = BlsScalar::random(rng);
    let opening = default_opening();

    let state = TestStateClient::new(notes, anchor, opening);
    let prover = SerdeProverClient {
        prover: TestProverClient,
    };

    Wallet::new(store, state, prover)
}

/// Returns obfuscated notes with the given value.
fn new_notes<Rng: RngCore + CryptoRng>(
    rng: &mut Rng,
    psk: &PublicSpendKey,
    note_values: &[u64],
) -> Vec<EnrichedNote> {
    note_values
        .iter()
        .map(|val| {
            let blinder = JubJubScalar::random(rng);
            (Note::new(rng, NoteType::Obfuscated, psk, *val, blinder), 0)
        })
        .collect()
}

/// An in-memory seed store.
#[derive(Debug)]
pub struct TestStore {
    seed: [u8; 64],
}

impl TestStore {
    /// Instantiate a new in-memory store with a random seed.
    fn new<Rng: RngCore + CryptoRng>(rng: &mut Rng) -> Self {
        let mut seed = [0; 64];
        rng.fill_bytes(&mut seed);
        Self { seed }
    }
}

impl Store for TestStore {
    type Error = ();

    fn get_seed(&self) -> Result<[u8; 64], Self::Error> {
        Ok(self.seed)
    }
}

/// A state client that always returns the same notes, anchor, and opening.
#[derive(Debug, Clone)]
pub struct TestStateClient {
    notes: Vec<EnrichedNote>,
    anchor: BlsScalar,
    opening: PoseidonOpening<(), POSEIDON_TREE_DEPTH, 4>,
}

impl TestStateClient {
    /// Create a new node given the notes, anchor, and opening we will return.
    fn new(
        notes: Vec<EnrichedNote>,
        anchor: BlsScalar,
        opening: PoseidonOpening<(), POSEIDON_TREE_DEPTH, 4>,
    ) -> Self {
        Self {
            notes,
            anchor,
            opening,
        }
    }
}

impl StateClient for TestStateClient {
    type Error = ();

    fn fetch_notes(
        &self,
        _: &ViewKey,
    ) -> Result<Vec<EnrichedNote>, Self::Error> {
        Ok(self.notes.clone())
    }

    fn fetch_anchor(&self) -> Result<BlsScalar, Self::Error> {
        Ok(self.anchor)
    }

    fn fetch_existing_nullifiers(
        &self,
        _: &[BlsScalar],
    ) -> Result<Vec<BlsScalar>, Self::Error> {
        Ok(vec![])
    }

    fn fetch_opening(
        &self,
        _: &Note,
    ) -> Result<PoseidonOpening<(), POSEIDON_TREE_DEPTH, 4>, Self::Error> {
        Ok(self.opening)
    }

    fn fetch_stake(&self, _pk: &PublicKey) -> Result<StakeInfo, Self::Error> {
        Ok(StakeInfo {
            amount: Some((100, 0)),
            reward: 0,
            counter: 0,
        })
    }
}

#[derive(Debug)]
pub struct TestProverClient;

impl ProverClient for TestProverClient {
    type Error = ();
    fn compute_proof_and_propagate(
        &self,
        utx: &UnprovenTransaction,
    ) -> Result<Transaction, Self::Error> {
        Ok(utx.clone().prove(Proof::default()))
    }

    fn request_stct_proof(
        &self,
        _fee: &Fee,
        _crossover: &Crossover,
        _value: u64,
        _blinder: JubJubScalar,
        _address: BlsScalar,
        _signature: Signature,
    ) -> Result<Proof, Self::Error> {
        Ok(Proof::default())
    }

    fn request_wfct_proof(
        &self,
        _commitment: JubJubAffine,
        _value: u64,
        _blinder: JubJubScalar,
    ) -> Result<Proof, Self::Error> {
        Ok(Proof::default())
    }
}

#[derive(Debug)]
pub struct RkyvProverClient {
    prover: TestProverClient,
}

impl ProverClient for RkyvProverClient {
    type Error = ();

    fn compute_proof_and_propagate(
        &self,
        utx: &UnprovenTransaction,
    ) -> Result<Transaction, Self::Error> {
        let utx_clone = utx.clone();

        let tx = utx_clone.prove(Proof::default());

        let bytes = rkyv::to_bytes::<_, 65536>(&tx)
            .expect("Encoding a tx should succeed")
            .to_vec();

        let decoded_tx: Transaction = rkyv::from_bytes(&bytes)
            .expect("Deserializing a transaction should succeed");

        assert_eq!(
            tx, decoded_tx,
            "Encoded and decoded transaction should be equal"
        );

        self.prover.compute_proof_and_propagate(utx)
    }

    fn request_stct_proof(
        &self,
        fee: &Fee,
        crossover: &Crossover,
        value: u64,
        blinder: JubJubScalar,
        address: BlsScalar,
        signature: Signature,
    ) -> Result<Proof, Self::Error> {
        self.prover.request_stct_proof(
            fee, crossover, value, blinder, address, signature,
        )
    }

    fn request_wfct_proof(
        &self,
        commitment: JubJubAffine,
        value: u64,
        blinder: JubJubScalar,
    ) -> Result<Proof, Self::Error> {
        self.prover.request_wfct_proof(commitment, value, blinder)
    }
}

#[derive(Debug)]
pub struct SerdeProverClient {
    prover: TestProverClient,
}

impl ProverClient for SerdeProverClient {
    type Error = ();

    fn compute_proof_and_propagate(
        &self,
        utx: &UnprovenTransaction,
    ) -> Result<Transaction, Self::Error> {
        let utx_bytes = utx.to_var_bytes();
        let utx_clone = UnprovenTransaction::from_slice(&utx_bytes)
            .expect("Successful deserialization");

        for (input, cinput) in
            utx.inputs().iter().zip(utx_clone.inputs().iter())
        {
            assert_eq!(input.nullifier(), cinput.nullifier());
            // assert_eq!(input.opening(), cinput.opening());
            assert_eq!(input.note(), cinput.note());
            assert_eq!(input.value(), cinput.value());
            assert_eq!(input.blinding_factor(), cinput.blinding_factor());
            assert_eq!(input.pk_r_prime(), cinput.pk_r_prime());
            // assert_eq!(input.signature(), cinput.signature());
        }

        for (output, coutput) in
            utx.outputs().iter().zip(utx_clone.outputs().iter())
        {
            assert_eq!(output, coutput);
        }

        assert_eq!(utx.anchor(), utx_clone.anchor());
        assert_eq!(utx.fee(), utx_clone.fee());
        assert_eq!(utx.crossover(), utx_clone.crossover());
        assert_eq!(utx.call(), utx_clone.call());

        self.prover.compute_proof_and_propagate(utx)
    }

    fn request_stct_proof(
        &self,
        fee: &Fee,
        crossover: &Crossover,
        value: u64,
        blinder: JubJubScalar,
        address: BlsScalar,
        signature: Signature,
    ) -> Result<Proof, Self::Error> {
        self.prover.request_stct_proof(
            fee, crossover, value, blinder, address, signature,
        )
    }

    fn request_wfct_proof(
        &self,
        commitment: JubJubAffine,
        value: u64,
        blinder: JubJubScalar,
    ) -> Result<Proof, Self::Error> {
        self.prover.request_wfct_proof(commitment, value, blinder)
    }
}
