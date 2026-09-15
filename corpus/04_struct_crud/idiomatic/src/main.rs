//! 4. Struct CRUD: a repository of users with create, read, update and delete methods.

#[derive(Debug, Clone, PartialEq)]
pub struct User {
    pub id: u32,
    pub name: String,
    pub active: bool,
}

pub struct Repo {
    users: Vec<User>,
    next_id: u32,
}

impl Repo {
    pub fn new() -> Self {
        Repo {
            users: Vec::new(),
            next_id: 1,
        }
    }

    pub fn create(&mut self, name: &str) -> u32 {
        let id = self.next_id;
        self.users.push(User {
            id,
            name: name.to_string(),
            active: true,
        });
        self.next_id += 1;
        id
    }

    pub fn get(&self, id: u32) -> Option<&User> {
        self.users.iter().find(|u| u.id == id)
    }

    pub fn rename(&mut self, id: u32, name: &str) -> bool {
        match self.users.iter_mut().find(|u| u.id == id) {
            Some(user) => {
                user.name = name.to_string();
                true
            }
            None => false,
        }
    }

    pub fn deactivate(&mut self, id: u32) {
        for user in &mut self.users {
            if user.id == id {
                user.active = false;
            }
        }
    }

    pub fn delete(&mut self, id: u32) -> bool {
        let before = self.users.len();
        self.users.retain(|u| u.id != id);
        self.users.len() != before
    }

    pub fn active_names(&self) -> Vec<String> {
        self.users
            .iter()
            .filter(|u| u.active)
            .map(|u| u.name.clone())
            .collect()
    }
}

fn main() {
    let mut repo = Repo::new();
    let ada = repo.create("Ada");
    let bob = repo.create("Bob");
    let _ = repo.create("Cy");
    println!("{:?}", repo.get(ada));
    println!("renamed: {}", repo.rename(bob, "Robert"));
    repo.deactivate(ada);
    println!("deleted missing: {}", repo.delete(99));
    println!("deleted cy: {}", repo.delete(3));
    println!("active: {:?}", repo.active_names());
}
