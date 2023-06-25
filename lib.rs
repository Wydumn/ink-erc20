#![cfg_attr(not(feature = "std"), no_std, no_main)]

#[ink::contract]
mod erc20 {
    use ink::storage::Mapping;

    #[ink(storage)]
    #[derive(Default)]
    pub struct Erc20 {
        total_supply: Balance,
        balances: Mapping<AccountId, Balance>,
        allowances: Mapping<(AccountId, AccountId), Balance>,
    }

    #[ink(event)]
    pub struct Transfer {
        #[ink(topic)]
        from: Option<AccountId>,
        #[ink(topic)]
        to: Option<AccountId>,
        value: Balance,
    }

    #[ink(event)]
    pub struct Approval {
        #[ink(topic)]
        from: AccountId,
        #[ink(topic)]
        to: AccountId,
        value: Balance,
    }

    #[derive(Debug, PartialEq, Eq, scale::Encode, scale::Decode)]
    #[cfg_attr(feature = "std", derive(scale_info::TypeInfo))]
    pub enum Error {
        BalanceTooLow,
        AllowanceToolow,
    }

    type Result<T> = core::result::Result<T, Error>;

    impl Erc20 {
        #[ink(constructor)]
        pub fn new(total_supply: Balance) -> Self {
            let mut balances = Mapping::new();
            balances.insert(Self::env().caller(), &total_supply);

            Self::env().emit_event(Transfer {
                from: None,
                to: Some(Self::env().caller()),
                value: total_supply,
            });

            Self {
                total_supply,
                balances,
                allowances: Default::default(),
            }
        }

        /// A message that can be called on instantiated contracts.
        /// This one flips the value of the stored `bool` from `true`
        /// to `false` and vice versa.
        #[ink(message)]
        pub fn total_supply(&self) -> Balance {
            self.total_supply
        }

        /// Simply returns the current value of our `bool`.
        #[ink(message)]
        pub fn balance_of(&self, who: AccountId) -> Balance {
            self.balances.get(&who).unwrap_or_default()
        }

        #[ink(message)]
        pub fn transfer(&mut self, to: AccountId, value: Balance) -> Result<()> {
            let sender = self.env().caller();
            self.transfer_helper(&sender, &to, value)
        }

        #[ink(message)]
        pub fn transfer_from(
            &mut self,
            from: AccountId,
            to: AccountId,
            value: Balance,
        ) -> Result<()> {
            let sender = self.env().caller();
            let allowance = self.allowances.get(&(from, sender)).unwrap_or_default();

            if allowance < value {
                return Err(Error::AllowanceToolow);
            }

            self.allowances.insert((from, sender), &(allowance - value));

            self.transfer_helper(&from, &to, value)
        }

        #[ink(message)]
        pub fn approve(&mut self, spender: AccountId, value: Balance) -> Result<()> {
            let sender = self.env().caller();
            self.allowances.insert((sender, spender), &value);

            self.env().emit_event(Approval {
                from: sender,
                to: spender,
                value,
            });

            Ok(())
        }

        pub fn transfer_helper(
            &mut self,
            from: &AccountId,
            to: &AccountId,
            value: Balance,
        ) -> Result<()> {
            let balance_from = self.balance_of(*from);
            let balance_to = self.balance_of(*to);

            if value > balance_from {
                return Err(Error::BalanceTooLow);
            }
            self.balances.insert(from, &(balance_from - value));
            self.balances.insert(to, &(balance_to + value));

            self.env().emit_event(Transfer {
                from: Some(*from),
                to: Some(*to),
                value,
            });

            Ok(())
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        type Event = <Erc20 as ::ink::reflect::ContractEventBase>::Type;

        #[ink::test]
        fn constructor_works() {
            let erc20 = Erc20::new(1000);
            assert_eq!(erc20.total_supply(), 1000);

            let accounts = ink::env::test::default_accounts::<ink::env::DefaultEnvironment>();
            assert_eq!(erc20.balance_of(accounts.alice), 1000);

            let emitted_events = ink::env::test::recorded_events().collect::<Vec<_>>();
            let event = emitted_events[0].clone();
            let decoded =
                <Event as scale::Decode>::decode(&mut &event.data[..]).expect("decoded error");
            match decoded {
                Event::Transfer(Transfer { from, to, value }) => {
                    assert!(from.is_none(), "mint from error");
                    assert_eq!(to, Some(accounts.alice));
                    assert_eq!(value, 1000);
                }
                _ => panic!("Expecting a Transfer event"),
            }
        }

        #[ink::test]
        fn transfer_should_work() {
            let mut erc20 = Erc20::new(1000);
            let accounts = ink::env::test::default_accounts::<ink::env::DefaultEnvironment>();
            let res = erc20.transfer(accounts.bob, 12);
            assert_eq!(res, Ok(()));
            assert_eq!(erc20.balance_of(accounts.alice), 1000 - 12);
            assert_eq!(erc20.balance_of(accounts.bob), 12);
        }

        #[ink::test]
        fn invalid_transfer_should_fail() {
            let mut erc20 = Erc20::new(1000);
            let accounts = ink::env::test::default_accounts::<ink::env::DefaultEnvironment>();
            ink::env::test::set_caller::<ink::env::DefaultEnvironment>(accounts.bob);
            let res = erc20.transfer(accounts.charlie, 12);
            assert_eq!(res, Err(Error::BalanceTooLow));
            /* let res = erc20.transfer(accounts.bob, 1001);
            assert_eq!(res, Err(Error::BalanceTooLow));
            assert_eq!(erc20.balance_of(accounts.alice), 1000);
            assert_eq!(erc20.balance_of(accounts.bob), 0); */
        }
    }

    #[cfg(all(test, feature = "e2e-tests"))]
    mod e2e_tests {
        use super::*;
        use ink_e2e::build_message;

        type E2EResult<T> = std::result::Result<T, Box<dyn std::error::Error>>;

        #[ink_e2e::test]
        async fn e2e_transfer(mut client: ink_e2e::Client<C, E>) -> E2EResult<()> {
            let total_supply = 1000;
            let constructor = Erc20Ref::new(total_supply);
            let contract_acc_id = client
                .instantiate("erc20", &ink_e2e::alice(), constructor, 0, None)
                .await
                .expect("instantiate failed")
                .account_id;
            let alice_acc = ink_e2e::account_id(ink_e2e::AccountKeyring::Alice);
            let bob_acc = ink_e2e::account_id(ink_e2e::AccountKeyring::Bob);

            let transfer_msg = build_message::<Erc20Ref>(
                contract_acc_id.clone(),
            ).call(|erc20| erc20.transfer(bob_acc.clone(), 2));

            let res = client.call(
                &ink_e2e::alice(), transfer_msg, 0, None
            ).await;

            let balance_of_msg = build_message::<Erc20Ref>(
                contract_acc_id.clone(),
            ).call(|erc20| erc20.balance_of(alice_acc.clone()));
            let balance_of_alice = client.call_dry_run(
                &ink_e2e::alice(), &balance_of_msg, 0, None
            ).await;

            assert_eq!(balance_of_alice.return_value(), 998);

            Ok(())
        }
    }
}
