// Rust Bitcoin Library
// Written in 2014 by
//   Andrew Poelstra <apoelstra@wpsoftware.net>
// To the extent possible under law, the author(s) have dedicated all
// copyright and related and neighboring rights to this software to
// the public domain worldwide. This software is distributed without
// any warranty.
//
// You should have received a copy of the CC0 Public Domain Dedication
// along with this software.
// If not, see <http://creativecommons.org/publicdomain/zero/1.0/>.
//

//! # Address Index
//!
//! Maintains an index from addresses to unspent outputs. It reduces size by
//! checking that the first byte of HMAC(wallet key, address outscript) is
//! zero, so that the index will be 1/256th the size of the utxoset in RAM.
//!

use std::collections::HashMap;
use collections::hash::sip::hash_with_keys;

use secp256k1::key::SecretKey;

use blockdata::transaction::{TxOut, PayToPubkeyHash};
use blockdata::utxoset::UtxoSet;
use blockdata::script::Script;
use network::constants::Network;
use wallet::address::Address;
use wallet::wallet::Wallet;
use util::hash::{DumbHasher, Sha256dHash};

/// The type of a wallet-spendable txout
#[deriving(Clone, PartialEq, Eq, Show)]
pub enum WalletTxOutType {
  /// Pay-to-address transaction redeemable using an ECDSA key
  PayToAddress(SecretKey),
  /// Undetermined
  Unknown
}


/// A txout that is spendable by the wallet
#[deriving(Clone, PartialEq, Eq, Show)]
pub struct WalletTxOut {
  /// The TXID of the transaction this output is part of 
  pub txid: Sha256dHash,
  /// The index of the output in its transaction
  pub vout: u32,
  /// The blockheight at which this output appeared in the blockchain
  pub height: u32,
  /// The actual output
  pub txo: TxOut,
  /// A classification of the output 
  pub kind: WalletTxOutType
}

/// An address index
#[deriving(Clone, PartialEq, Eq, Show)]
pub struct AddressIndex {
  tentative_index: HashMap<Script, Vec<WalletTxOut>>,
  index: HashMap<(Sha256dHash, u32), Vec<WalletTxOut>, DumbHasher>,
  network: Network,
  k1: u64,
  k2: u64
}

impl AddressIndex {
  /// Creates a new address index from a wallet (which provides an authenticated
  /// hash function for prefix filtering) and UTXO set (which is what gets filtered).
  pub fn new(utxo_set: &UtxoSet, wallet: &Wallet) -> AddressIndex {
    let (k1, k2) = wallet.siphash_key();
    let mut ret = AddressIndex {
      tentative_index: HashMap::with_capacity(utxo_set.n_utxos() / 256),
      index: HashMap::with_hasher(DumbHasher),
      network: wallet.network(),
      k1: k1,
      k2: k2
    };
    for (key, idx, txo, height) in utxo_set.iter() {
      if ret.admissible_txo(txo) {
        let new = WalletTxOut {
          txid: key,
          vout: idx,
          height: height,
          txo: txo.clone(),
          kind: Unknown
        };
        ret.tentative_index.find_or_insert(txo.script_pubkey.clone(), vec![]).push(new);
      }
    }
    ret
  }

  /// 
  #[inline]
  pub fn index_wallet_txo(&mut self, wtx: &WalletTxOut, kind: WalletTxOutType) {
    let mut new = wtx.clone();
    new.kind = kind;
    self.index.find_or_insert((wtx.txid, wtx.vout), vec![]).push(new);
  }

  /// A filtering function used for creating a small address index.
  #[inline]
  pub fn admissible_address(&self, addr: &Address) -> bool {
    hash_with_keys(self.k1, self.k2, &addr.as_slice()) & 0xFF == 0
  }

  /// A filtering function used for creating a small address index.
  #[inline]
  pub fn admissible_txo(&self, out: &TxOut) -> bool {
    match out.classify(self.network) {
      PayToPubkeyHash(addr) => self.admissible_address(&addr),
      _ => false
    }
  }

  /// Lookup a txout by its scriptpubkey. Returns a slice because there
  /// may be more than one for any given scriptpubkey.
  #[inline]
  pub fn find_by_script<'a>(&'a self, pubkey: &Script) -> &'a [WalletTxOut] {
    self.tentative_index.find(pubkey).map(|v| v.as_slice()).unwrap_or(&[])
  }
}

