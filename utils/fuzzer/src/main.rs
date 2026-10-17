use lb_rs::blocking::Lb;
use lb_rs::model::core_config::Config;
use lb_rs::model::file_metadata::FileType;
use uuid::Uuid;
use rand::seq::SliceRandom;

struct Trial{
    actions: Vec<Actions>,
    lb:Lb,
    file_id: Option<Uuid>,
    destination_id: Option<Uuid>,
}
#[derive(Clone, Copy)]
enum Actions {

    CreateAccount,
    CreateFile,
    MoveFile,
    WriteFile,

}


impl Trial {
    fn check_account(&mut self){
        if(!self.has_account()){
            let username = random::<u64>().to_string();
            self.lb.create_account(&username,"url",true).unwrap();
            self.has_account = true;
            println!("Created account: {username}");
        }
    }

    fn check_file(&mut self){
        self.check_account();
        if self.file_id.is_none(){
            let root = self.lb.root().unwrap();
            let file = self.lb.create_file("doc1", &root.id,FileType::Document).unwrap();
            self.file_id=Some(file.id);
            println!("Created file.");
        }

    }

    fn check_destination(&mut self) {
        self.check_account();
        if self.destination_id.is_none() {
            let root = self.lb.root().unwrap();
            let destination = self.lb.create_file("folder", &root.id, FileType::Folder).unwrap();
            self.destination_id = Some(destination.id);
            println!("Created destination folder.");
        }
    }
    fn action(&mut self){
        for action in &self.actions.clone(){
            match action{
                //generate random username
                //run server locally and point program at it
                Actions::CreateAccount=>{
                    self.check_account();
                },
                Actions::CreateFile=>{
                        self.check_file();
                },
                Actions::MoveFile=>{
                  self.check_file();
                    self.check_destination();
                    self.lb.move_file(&self.file_id.unwrap(), &self.destination_id.unwrap()).unwrap();
                    println!("Moved document");
                },
                Actions::WriteFile=>{
                    self.check_file();
                    self.lb.write_document(self.file_id.unwrap(),b"Hello world!").unwrap();
                    println!("Wrote to document.");
                }
                _ => {}
            }
        }

    }
    fn new() -> Self{
        let actions = [
            Actions::CreateAccount,
            Actions::CreateFile,
            Actions::MoveFile,
            Actions::WriteFile,

        ];
        let mut rng = rand::thread_rng();
        let n = 4;
        let mut v = vec![];
        for x in actions.choose_multiple(&mut rng,n){
            v.push(*x);
        }
        Self{
            actions: v,
            lb: Lb::init(Config::cli_config(".")).unwrap(),
            file_id: None,
            destination_id: None,
        }
    }
}

fn main() {

    let mut trial1 = Trial::new();
   trial1.action();

}
