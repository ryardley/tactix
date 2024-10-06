use std::time::Duration;

use async_trait::async_trait;
use tactix::{Actor, Context, Handler, Message};

mod addr;
mod addr_sender;
mod context;
mod envelope;
mod envelope_inner;
mod recipient;
mod tactix;
mod traits;

#[derive(Debug)]
pub struct Increment;
impl Message for Increment {
    type Response = ();
}

#[derive(Debug)]
pub struct Decrement;
impl Message for Decrement {
    type Response = ();
}

#[derive(Debug)]
pub struct GetCount;
impl Message for GetCount {
    type Response = u64;
}
#[derive(Clone)]
pub struct Counter {
    count: u64,
}

impl Counter {
    pub fn new() -> Self {
        Self { count: 0 }
    }
}

impl Actor for Counter {
    type Context = Context<Self>;
}

#[async_trait]
impl Handler<Increment> for Counter {
    async fn handle(&mut self, _msg: Increment) {
        println!("INC");
        self.count += 1;
    }
}

#[async_trait]
impl Handler<Decrement> for Counter {
    async fn handle(&mut self, _msg: Decrement) {
        println!("DEC");
        self.count -= 1;
    }
}

#[async_trait]
impl Handler<GetCount> for Counter {
    async fn handle(&mut self, _: GetCount) -> u64 {
        println!("GET");
        let s = self.count;
        println!("SENDING: {}", s);
        s
    }
}

//////////////////////////

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
    type Response = (u64,u64,u64);
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
    async fn handle(&mut self, msg: Deposit) {
        tokio::time::sleep(Duration::from_millis(6)).await;
        self.balance += msg.0;
        self.total_deposits += msg.0;
        println!("Deposit: {}. New balance: {}", msg.0, self.balance);
    }
}
#[async_trait]
impl Handler<Withdraw> for BankAccount {
    async fn handle(&mut self, msg: Withdraw) -> Result<(), String> {
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
    async fn handle(&mut self, _msg: GetAccountInfo) -> (u64,u64,u64) {
        (self.balance,self.total_deposits,self.total_withdrawals)
    }
}
#[async_trait]
impl Handler<GetBalance> for BankAccount {
    async fn handle(&mut self, _msg: GetBalance) -> u64 {
        self.balance
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use crate::{
        recipient::Recipient, tactix::{Actor, Context, Handler, Message, Sender}, BankAccount, Counter, Decrement, Deposit, GetAccountInfo, GetCount, Increment, Withdraw
    };
    use tokio::time::sleep;

    #[tokio::test]
    async fn test_counter() -> Result<(), Box<String>> {
        let addr = Counter::new().start();
        let incrementor: Recipient<Increment> = addr.clone().recipient();
        let decrementor: Recipient<Decrement> = addr.clone().recipient();
        addr.do_send(Increment);
        incrementor.do_send(Increment);
        addr.do_send(Increment);
        addr.do_send(Increment);
        addr.do_send(Decrement);
        decrementor.do_send(Decrement);
        let count = addr.send(GetCount).await.unwrap();

        assert_eq!(count, 2);
        Ok(())
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
