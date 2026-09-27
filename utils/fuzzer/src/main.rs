use rand::Rng;
use rand::seq::SliceRandom;

enum actions {
    CreateAccount,
    DeleteAccount,
    GetPassword,
    GetUsername,
}

impl actions {
    fn action(&self){
        match self{
            actions::GetPassword=>println!("password"),
            actions::GetUsername=>println!("username"),
            actions::CreateAccount=>println!("account created"),
            actions::DeleteAccount=>println!("account deleted"),
        }
    }
}
fn main() {
    let actions = [
        actions::CreateAccount,
        actions::DeleteAccount,
        actions::GetPassword,
        actions::GetUsername,
    ];
    let mut rng = rand::thread_rng();
    let n = 2;
    for x in actions.choose_multiple(&mut rng,n){
        x.action();
    }
}
