use std::{net::SocketAddr, sync::Arc, io};
use tokio::{net::UdpSocket, sync::mpsc};


#[tokio::main]
async fn main() -> io::Result<()> {
    let connection_uri = "127.0.0.1:7878";
    let socket = UdpSocket::bind(connection_uri).await?;
    let r = Arc::new(socket);
    let s = r.clone();
    let (tx, mut rx) = mpsc::channel::<RawMessage>(1_000);

    tokio::spawn(async move {
        let mut users: Vec<User> = Vec::new();
        while let Some(raw_message) = rx.recv().await {
            println!("Sending the response for {:?} : {:?}", raw_message.from, raw_message.content);
            users = handle_message(&s, users, raw_message).await.unwrap();
        }
    });


    println!("waiting for messages");
    let mut buf = [0; 1024];
    loop {
        let (read_bytes, remote_addr) = r.recv_from(&mut buf).await?;
        let raw_messge = RawMessage::new(remote_addr, buf, read_bytes);
        tx.send(raw_messge).await.unwrap();
    }
}


#[derive(Eq, Hash, PartialEq, Clone)]
struct User {
    addr: SocketAddr,
    id: usize,
    name: Option<String>,
    color: Option<u8>,
}

impl User {
    fn new(addr: SocketAddr, id: usize, name: String) -> User {
        return User {
            addr,
            id,
            name: Some(name),
            color: None,
        };
    }
}

#[derive(Debug)]
struct RawMessage {
    from: SocketAddr,
    content: Vec<u8>,
}

impl RawMessage {
    fn new(from: SocketAddr, content: [u8; 1024], read_bytes: usize) -> Self {
        Self {
            from,
            content: content[..read_bytes].to_vec(),
        }
    }
}

enum ResponseType {
    Text,
    UserName,
}

impl From<u8> for ResponseType {
    fn from(v: u8) -> Self {
        if v == 1 {
            return ResponseType::UserName;
        }
        return ResponseType::Text;
    }
}

impl Into<u8> for ResponseType {
    fn into(self) -> u8 {
        match self {
            ResponseType::Text => return 0u8,
            ResponseType::UserName => return 1u8,
        }
    }
}

struct Response {
    response_type: ResponseType,
    user_id: u8,
    color: u8,
    text: Vec<u8>,
}

impl Response {
    fn new(response_type: ResponseType, user: User, message: Vec<u8>) -> Self {
        Response {
            response_type,
            user_id: user.id as u8,
            color: user.color.unwrap_or(0),
            text: message,
        }
    }

    fn buf(self: Self) -> Vec<u8> {
        let header: &[u8] = &[self.response_type.into(), self.user_id, self.color];
        let buf: &[u8] = &[header, self.text.as_slice()].concat();
        return buf.into();
    }
}

impl From<Vec<u8>> for Response {
    fn from(data: Vec<u8>) -> Self {
        let response_type = ResponseType::from(data[0]);
        let user_id: u8 = data[1];
        let color = data[2];
        let text = &data[2..];

        return Self {
            response_type,
            user_id,
            color,
            text: text.to_vec(),
        };
    }
}

enum MessageType {
    Unknown,
    Login(SocketAddr, String),
    Color(User, Vec<u8>),
    UserQuery(User, User),
    Text(User, Vec<u8>),
}

impl MessageType {
    fn new(mut message: RawMessage, users: &mut Vec<User>) -> Self {
        if message.content.is_empty() {
            return MessageType::Unknown;
        }
        let msg_type = message.content.remove(0);
        match msg_type {
            1u8 => {
                return MessageType::Login(
                    message.from,
                    String::from_utf8(message.content).unwrap(),
                );
            }
            2u8 => {
                let user = match find_user_by_addr(message.from, users.to_vec()) {
                    Some(user) => user,
                    None => return MessageType::Unknown,
                };
                return MessageType::Color(user.to_owned(), message.content);
            }
            3u8 => {
                let user = match find_user_by_addr(message.from, users.to_vec()) {
                    Some(user) => user,
                    None => return MessageType::Unknown,
                };

                let id = match message.content.first() {
                    Some(v) => *v as usize,
                    None => return MessageType::Unknown,
                };

                let query_user = match find_user_by_id(id, users.to_vec()) {
                    Some(user) => user,
                    None => return MessageType::Unknown,
                };
                return MessageType::UserQuery(user.to_owned(), query_user);
            }
            4u8 => {
                let user = match find_user_by_addr(message.from, users.to_vec()) {
                    Some(user) => user,
                    None => return MessageType::Unknown,
                };
                return MessageType::Text(user.to_owned(), message.content);
            }
            _ => {
                return MessageType::Unknown;
            }
        }
    }
}

fn find_user_by_addr(addr: SocketAddr, users: Vec<User>) -> Option<User> {
    for user in users {
        if user.addr == addr {
            return Some(user);
        }
    }
    return None;
}

fn find_user_by_id(id: usize, users: Vec<User>) -> Option<User> {
    for user in users {
        if user.id == id {
            return Some(user);
        }
    }
    return None;
}

async fn handle_message(socket: &UdpSocket, mut users: Vec<User>, raw_messge: RawMessage) -> io::Result<Vec<User>> {
    match MessageType::new(raw_messge, &mut users) {
        MessageType::Login(addr, user_name) => {
            println!("User from {}, logged in with name {}", addr, user_name);
            users.push(User::new(addr, users.len().try_into().unwrap(), user_name));
            return Ok(users);
        }
        MessageType::Color(from_user, message) => {
            let new_color = match message.first() {
                Some(color) => Some(*color),
                None => Some(0),
            };

            let mut new_user = User::new(from_user.addr, from_user.id, from_user.name.unwrap());
            new_user.color = new_color;
            println!("user set color to {}", new_color.unwrap());
            users[from_user.id] = new_user;
            return Ok(users);
        }
        MessageType::Text(from_user, message) => {
            println!("we got text message");
            let return_msg = Response::new(ResponseType::Text, from_user.to_owned(), message);
            let b = return_msg.buf();
            let buf = b.as_slice();

            for user in &users {
                if user.id == from_user.id {
                    continue;
                }
                socket.send_to(buf, user.addr).await?;
            }
            return Ok(users);
        }
        MessageType::UserQuery(from_user, user) => {
            println!("we got user query message");
            let return_msg = Response::new(
                ResponseType::UserName,
                from_user.to_owned(),
                user.name.unwrap().into_bytes(),
            );
            let b = return_msg.buf();
            let buf = b.as_slice();

            socket.send_to(buf, from_user.addr).await?;
            return Ok(users);
        }
        MessageType::Unknown => Ok(users),
    }
}
