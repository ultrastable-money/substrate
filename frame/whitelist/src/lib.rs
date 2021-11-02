// This file is part of Substrate.

// Copyright (C) 2017-2021 Parity Technologies (UK) Ltd.
// SPDX-License-Identifier: Apache-2.0

// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// 	http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! # Whitelist Pallet
//!
//! - [`Config`]
//! - [`Call`]
//!
//! ## Overview
//!
//! Allow some origin to whitelist some call, and another origin to dispatch them with the root
//! origin.

#![cfg_attr(not(feature = "std"), no_std)]

use sp_runtime::traits::Dispatchable;
use sp_std::prelude::*;

use codec::{Decode, Encode, MaxEncodedLen};
use frame_support::{ensure, traits::PreimageHandler, weights::GetDispatchInfo};
use scale_info::TypeInfo;

use frame_support::pallet_prelude::*;
use frame_system::pallet_prelude::*;

pub use pallet::*;

#[derive(Clone, Encode, Decode, TypeInfo, MaxEncodedLen)]
pub struct Preimage<BoundedVec, Balance, AccountId> {
	preimage: BoundedVec,
	deposit: Option<(AccountId, Balance)>,
}

#[frame_support::pallet]
pub mod pallet {
	use super::*;

	#[pallet::config]
	pub trait Config: frame_system::Config {
		/// The overarching event type.
		type Event: From<Event<Self>> + IsType<<Self as frame_system::Config>::Event>;
		/// Required origin for whitelisting a call.
		type WhitelistOrigin: EnsureOrigin<Self::Origin>;
		/// Required origin for dispatching whitelisted call with root origin.
		type DispatchWhitelistedOrigin: EnsureOrigin<Self::Origin>;
		/// The handler of pre-images.
		type PreimageHandler: frame_support::traits::PreimageHandler<Self::Hash>;
	}

	#[pallet::pallet]
	#[pallet::generate_store(pub(super) trait Store)]
	#[pallet::generate_storage_info]
	pub struct Pallet<T>(PhantomData<T>);

	#[pallet::event]
	#[pallet::generate_deposit(pub(super) fn deposit_event)]
	pub enum Event<T: Config> {
		CallWhitelisted { call_hash: T::Hash },
		WhitelistedCallRemoved { call_hash: T::Hash },
		WhitelistedCallDispatched { call_hash: T::Hash },
	}

	#[pallet::error]
	pub enum Error<T> {
		UnavailablePreImage,
		UndecodableCall,
		InvalidCallWeightWitness,
		CallIsNotWhitelisted,
	}

	#[pallet::storage]
	pub type WhitelistedCall<T: Config> = StorageMap<_, Twox64Concat, T::Hash, (), OptionQuery>;

	#[pallet::call]
	impl<T: Config> Pallet<T> {
		#[pallet::weight(0)]
		pub fn whitelist_call(origin: OriginFor<T>, call_hash: T::Hash) -> DispatchResult {
			T::WhitelistOrigin::ensure_origin(origin)?;

			WhitelistedCall::<T>::insert(call_hash, ());
			T::PreimageHandler::request_preimage(call_hash.clone());

			Self::deposit_event(Event::<T>::CallWhitelisted { call_hash });

			Ok(())
		}

		#[pallet::weight(0)]
		pub fn remove_whitelisted_call(origin: OriginFor<T>, call_hash: T::Hash) -> DispatchResult {
			T::WhitelistOrigin::ensure_origin(origin)?;

			WhitelistedCall::<T>::take(call_hash).ok_or(Error::<T>::CallIsNotWhitelisted)?;

			T::PreimageHandler::clear_preimage(call_hash.clone());

			Self::deposit_event(Event::<T>::WhitelistedCallRemoved { call_hash });

			Ok(())
		}

		#[pallet::weight(0)]
		pub fn dispatch_whitelisted_call(
			origin: OriginFor<T>,
			call_hash: T::Hash,
			call_weight_witness: Weight,
		) -> DispatchResultWithPostInfo {
			T::DispatchWhitelistedOrigin::ensure_origin(origin)?;

			WhitelistedCall::<T>::take(call_hash).ok_or(Error::<T>::CallIsNotWhitelisted)?;

			let call = T::PreimageHandler::get_preimage(call_hash)
				.ok_or(Error::<T>::UnavailablePreImage)?;

			let call = T::Call::decode(&mut &call[..]).map_err(|_| Error::<T>::UndecodableCall)?;

			ensure!(
				call.get_dispatch_info().weight <= call_weight_witness,
				Error::<T>::InvalidCallWeightWitness
			);
			let result = call.dispatch(frame_system::Origin::<T>::Root.into());

			Self::deposit_event(Event::<T>::WhitelistedCallDispatched { call_hash });

			result
		}
	}
}
