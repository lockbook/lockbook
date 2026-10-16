use lb_rs::blocking::Lb;
use lb_rs::model::core_config::Config;
use lb_rs::model::file_metadata::FileType;
use rand::{random, Rng};
use uuid::Uuid;
use rand::seq::SliceRandom;

struct Trial{
    actions: Vec<Actions>,
    lb:Lb,
    file_id: Option<Uuid>,
    destination_id: Option<Uuid>,

    file: ()
}
#[derive(Clone, Copy)]
enum Actions {

    CreateAccount,
    CreateFile,
    MoveFile,
    WriteFile,

}
//download lockbook from windows
//push changes
//migrate notes

impl Trial {
    fn action(&mut self){

        for action in &self.actions{
            match action{
                //generate random username
                //run server locally and point program at it
                Actions::CreateAccount=>{
                    let username = random::<u64>();
                    self.lb.create_account("user1","url",true).unwrap();
                    println!("Created account: {username}");
                },
                Actions::CreateFile=>{
                    let root = self.lb.get_root();
                    let file = self.lb.create_file("doc1", &root.unwrap().id,FileType::Document);
                    self.file_id=Some(file.unwrap().id);
                    println!("Created file.");
                    let destination = self.lb.create_file("folder", &root.unwrap().id,FileType::Folder).unwrap();
                    self.destination_id=Some(destination.id);
                    println!("Created destination folder.");
                },
                Actions::MoveFile=>{
                  self.lb.move_file(self.file_id, self.destination_id).unwrap();
                    println!("Moved document");
                },
                Actions::WriteFile=>{
                    self.lb.write_document(self.file_id.expect("REASON"),b"Hello world!").unwrap();
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
            file: (),
        }
    }
}

fn main() {

    let mut trial1 = Trial::new();
   trial1.action();

}
