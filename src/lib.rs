//! A simple actor model implementation based on Tokio
//! ## Tactix
//! Tactix is an erganomic Actor Model framework for Rust inspired by  [Actix](https://github.com/actix/actix)
//!
//! [Actix](https://github.com/actix/actix) provides a great API for working with actors but holds
//! a large amount of technical debt and confusing code within it's implementation.
//! Actix as an early solution for asynchrony originally built out it's own async runtime before
//! moving to Tokio as a default runtime and as a consequence holds a fair amount of baggage from
//! that time.
//!
//! Alice Ryhl a maintainer of tokio wrote a [great article on creating an actor model with tokio](https://ryhl.io/blog/actors-with-tokio) where it is outlined how to create an actor model system using tokio channels. This however leads to relatively verbose code as events must be discriminated.
//!
//! Tactix attempts to apply some techniques from [Alice Ryhl's article](https://ryhl.io/blog/actors-with-tokio/) and combine them with Actix's handler syntax whilst enabling safe async handlers in order to get the best of both worlds.
//!
//! ## Installation
//! You can install Tactix with Cargo:
//! ```bash
//! cargo add tactix
//! ```
//! ## Creating an Actor
//! You can create an actor by simply implementing the `Actor` trait on a struct.
//! ```
//! use tactix::{Actor,Message,Context,Sender,Recipient,Handler};
//! use async_trait::async_trait;
//! use tokio::time::{sleep, Duration};
//!
//! // Define an Actor struct
//! struct Counter {
//!   count: u64
//! }
//!
//! // Implement the Actor trait on the struct
//! impl Actor for Counter {
//!   type Context = Context<Self>;
//! }
//!
//! // Define a message
//! struct Increment;
//! impl Message for Increment {
//!   type Response = ();
//! }
//!
//! // Define a handler for the message.
//! // Note: this requires an async_trait macro!
//! #[async_trait]
//! impl Handler<Increment> for Counter {
//!   async fn handle(&mut self, msg:Increment, _:Self::Context) {
//!     println!("Increment");
//!     self.count += 1;
//!   }
//! }
//!
//! // We can do the same for Decrement
//! struct Decrement;
//! impl Message for Decrement {
//!   type Response = ();
//! }
//!
//! #[async_trait]
//! impl Handler<Decrement> for Counter {
//!   async fn handle(&mut self, msg:Decrement, _:Self::Context) {
//!     println!("Decrement");
//!     self.count -= 1;
//!   }
//! }
//!
//! // An event for getting the count
//! struct GetCount;
//! impl Message for GetCount {
//!   type Response = u64;
//! }
//!
//! #[async_trait]
//! impl Handler<GetCount> for Counter {
//!   async fn handle(&mut self, msg:GetCount, _:Self::Context) -> u64 {
//!     println!("GetCount");
//!     self.count
//!   }
//! }
//!
//! #[tokio::main]
//! async fn main() -> Result<(),String> {
//!   // Construct the actor
//!   let counter = Counter { count: 0 };
//!
//!   // Start the actor
//!   let addr = counter.start();
//!
//!   // Let's get a Recipient of the decrement message
//!   // This is often useful when injecting actors as dependencies
//!   let decrementor:Recipient<Decrement> = addr.clone().recipient();
//!
//!   // Tell the actor to `Increment`
//!   addr.do_send(Increment);
//!   addr.do_send(Increment);
//!   addr.do_send(Increment);
//!
//!   // And decrement
//!   addr.do_send(Decrement);
//!   decrementor.do_send(Decrement);
//!
//!   // Wait for all messages to arrive
//!   sleep(Duration::from_millis(1)).await;
//!
//!   // To receive a response we use the send async method
//!   let count = addr.send(GetCount).await.map_err(|e|e.to_string())?;
//!   assert_eq!(count, 1);
//!   Ok(())
//! }
//! ```

pub use recipient::Recipient;
pub use tactix::{Actor, Addr, Context, Handler, Message};
pub use traits::Sender;
mod addr;
mod addr_sender;
mod context;
mod envelope;
mod envelope_inner;
mod recipient;
mod tactix;
mod traits;

//////////////////////////

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use crate::{
        tactix::{Actor, Sender},
        Context, Handler, Message,
    };
    use async_trait::async_trait;
    use tokio::time::sleep;

    struct Deposit(u64);
    impl Message for Deposit {
        type Response = ();
    }

    struct Withdraw(u64);
    impl Message for Withdraw {
        type Response = Result<(), String>;
    }

    struct GetBalance;
    impl Message for GetBalance {
        type Response = u64;
    }

    struct GetAccountInfo;
    impl Message for GetAccountInfo {
        type Response = (u64, u64, u64);
    }

    #[derive(Clone)]
    struct BankAccount {
        balance: u64,
        total_deposits: u64,
        total_withdrawals: u64,
    }

    impl BankAccount {
        fn new(initial_balance: u64) -> Self {
            Self {
                balance: initial_balance,
                total_deposits: 0,
                total_withdrawals: 0,
            }
        }
    }

    impl Actor for BankAccount {
        type Context = Context<Self>;
    }

    #[async_trait]
    impl Handler<Deposit> for BankAccount {
        async fn handle(&mut self, msg: Deposit, _:Self::Context) {
            tokio::time::sleep(Duration::from_millis(6)).await;
            self.balance += msg.0;
            self.total_deposits += msg.0;
            println!("Deposit: {}. New balance: {}", msg.0, self.balance);
        }
    }
    #[async_trait]
    impl Handler<Withdraw> for BankAccount {
        async fn handle(&mut self, msg: Withdraw, _:Self::Context) -> Result<(), String> {
            if self.balance >= msg.0 {
                tokio::time::sleep(Duration::from_millis(10)).await;
                self.balance -= msg.0;
                self.total_withdrawals += msg.0;
                println!("Withdrawal: {}. New balance: {}", msg.0, self.balance);
                Ok(())
            } else {
                Err(format!(
                    "Insufficient funds. Current balance: {}",
                    self.balance
                ))
            }
        }
    }

    #[async_trait]
    impl Handler<GetAccountInfo> for BankAccount {
        async fn handle(&mut self, _msg: GetAccountInfo, _:Self::Context) -> (u64, u64, u64) {
            println!("GetAccountInfo!");
            (self.balance, self.total_deposits, self.total_withdrawals)
        }
    }
    #[async_trait]
    impl Handler<GetBalance> for BankAccount {
        async fn handle(&mut self, _msg: GetBalance, _:Self::Context) -> u64 {
            self.balance
        }
    }

    #[tokio::test]
    async fn test_bank_account_race_condition() {
        let initial_balance = 1000;
        let account = BankAccount::new(initial_balance).start();

        let deposit_amount = 100;
        let withdraw_amount = 200;
        let num_operations = 5;

        // Spawn multiple tasks to deposit and withdraw concurrently
        let deposit_task = tokio::spawn({
            let account = account.clone();
            async move {
                for _ in 0..num_operations {
                    account.do_send(Deposit(deposit_amount));
                    sleep(Duration::from_millis(3)).await;
                }
            }
        });

        let withdraw_task = tokio::spawn({
            let account = account.clone();
            async move {
                for _ in 0..num_operations {
                    account.do_send(Withdraw(withdraw_amount));
                    sleep(Duration::from_millis(9)).await;
                }
            }
        });

        // Wait for both tasks to complete
        let _ = tokio::join!(deposit_task, withdraw_task);

        // Get the final account info
        let (final_balance, total_deposits, total_withdrawals) =
            account.send(GetAccountInfo).await.unwrap();

        // Assertions to check for race conditions
        let expected_deposits = deposit_amount * num_operations;
        let expected_withdrawals = withdraw_amount * num_operations;
        let expected_balance = initial_balance + expected_deposits - expected_withdrawals;

        assert_eq!(
            total_deposits, expected_deposits,
            "Total deposits don't match expected value"
        );
        assert_eq!(
            total_withdrawals, expected_withdrawals,
            "Total withdrawals don't match expected value"
        );

        // This assertion might fail due to race condition
        assert_eq!(
            final_balance, expected_balance,
            "Final balance doesn't match expected value"
        );

        // This assertion checks if the balance is consistent with deposits and withdrawals
        assert_eq!(
            final_balance,
            initial_balance + total_deposits - total_withdrawals,
            "Balance is inconsistent with recorded deposits and withdrawals"
        );
    }
}
