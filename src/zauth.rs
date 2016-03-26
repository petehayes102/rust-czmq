//! Module: czmq-zauth

use {czmq_sys, ZActor, ZMsg};
use std::result;

// Generic error code "-1" doesn't map to an error message, so just
// return an empty tuple.
pub type Result<T> = result::Result<T, ()>;

pub struct ZAuth {
    zactor: ZActor,
}

impl ZAuth {
    pub fn new() -> Result<ZAuth> {
        Ok(ZAuth {
            zactor: try!(ZActor::new(czmq_sys::zauth)),
        })
    }

    pub fn allow(&self, address: &str) -> Result<()> {
        let msg = ZMsg::new();
        try!(msg.addstr("ALLOW"));
        try!(msg.addstr(address));

        try!(self.zactor.send(msg));
        self.zactor.sock().wait()
    }

    pub fn deny(&self, address: &str) -> Result<()> {
        let msg = ZMsg::new();
        try!(msg.addstr("DENY"));
        try!(msg.addstr(address));

        try!(self.zactor.send(msg));
        self.zactor.sock().wait()
    }

    pub fn load_plain(&self, filename: &str) -> Result<()> {
        let msg = ZMsg::new();
        try!(msg.addstr("PLAIN"));
        try!(msg.addstr(filename));

        try!(self.zactor.send(msg));
        self.zactor.sock().wait()
    }

    pub fn load_curve(&self, location: Option<&str>) -> Result<()> {
        let msg = ZMsg::new();
        try!(msg.addstr("CURVE"));

        if let Some(loc) = location {
            try!(msg.addstr(loc));
        } else {
            try!(msg.addstr("*"));
        }

        try!(self.zactor.send(msg));
        self.zactor.sock().wait()
    }

    // XXX This is unimplemented upstream, so it's just a placeholder.
    pub fn load_gssapi(&self) -> Result<()> {
        unimplemented!();
    }

    pub fn verbose(&self) -> Result<()> {
        try!(self.zactor.send_str("VERBOSE"));
        self.zactor.sock().wait()
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;
    use std::thread::sleep;
    use std::time::Duration;
    use super::*;
    use tempdir::TempDir;
    use tempfile::NamedTempFile;
    use {zmq, ZCert, ZSock, zsys_init};

    // There can only be one ZAuth instance per context as each ZAuth
    // instance binds to the same inproc endpoint. The simplest way
    // around this limitation is to run all the tests in sequence.
    #[test]
    fn test_zauth() {
        zsys_init();

        test_verbose();
        test_allow_deny();
        test_plain();
        test_curve();
    }

    fn test_verbose() {
        let zauth = ZAuth::new().unwrap();
        assert!(zauth.verbose().is_ok());
    }

    fn test_allow_deny() {
        let server = ZSock::new(zmq::PULL);
        server.set_zap_domain("compuglobalhypermega.net");
        server.set_rcvtimeo(100);

        let client = ZSock::new(zmq::PUSH);
        client.set_linger(100);
        client.set_sndtimeo(100);

        let zauth = ZAuth::new().unwrap();

        assert!(zauth.deny("127.0.0.1").is_ok());
        sleep(Duration::from_millis(100));

        let port = server.bind("tcp://127.0.0.1:*[60000-]").unwrap();

        client.connect(&format!("tcp://127.0.0.1:{}", port)).unwrap();
        sleep(Duration::from_millis(100));

        client.send_str("test").unwrap();
        assert!(server.recv_str().is_err());

        assert!(zauth.allow("127.0.0.1").is_ok());
        sleep(Duration::from_millis(100));

        client.connect(&format!("tcp://127.0.0.1:{}", port)).unwrap();
        sleep(Duration::from_millis(100));

        client.send_str("test").unwrap();
        assert_eq!(server.recv_str().unwrap().unwrap(), "test");
    }

    fn test_plain() {
        let zauth = ZAuth::new().unwrap();

        let server = ZSock::new(zmq::PULL);
        server.set_zap_domain("sky.net");
        server.set_plain_server(true);
        server.set_rcvtimeo(100);
        let port = server.bind("tcp://127.0.0.1:*[60000-]").unwrap();

        let client = ZSock::new(zmq::PUSH);
        client.set_plain_username("moo");
        client.set_plain_password("cow");
        client.set_linger(100);
        client.set_sndtimeo(100);
        client.connect(&format!("tcp://127.0.0.1:{}", port)).unwrap();

        sleep(Duration::from_millis(100));

        client.send_str("test").unwrap();
        assert!(server.recv_str().is_err());

        let mut passwd_file = NamedTempFile::new().unwrap();
        passwd_file.write_all("moo=cow\n".as_bytes()).unwrap();

        zauth.load_plain(passwd_file.path().to_str().unwrap()).unwrap();
        sleep(Duration::from_millis(100));

        client.connect(&format!("tcp://127.0.0.1:{}", port)).unwrap();
        sleep(Duration::from_millis(100));

        client.send_str("test").unwrap();
        assert_eq!(server.recv_str().unwrap().unwrap(), "test");
    }

    fn test_curve() {
        let zauth = ZAuth::new().unwrap();

        let server = ZSock::new(zmq::PULL);
        let server_cert = ZCert::new().unwrap();
        server_cert.zapply(&server);
        server.set_zap_domain("sky.net");
        server.set_curve_server(true);
        server.set_rcvtimeo(100);
        let port = server.bind("tcp://127.0.0.1:*[60000-]").unwrap();

        let client = ZSock::new(zmq::PUSH);
        let client_cert = ZCert::new().unwrap();
        client_cert.zapply(&client);
        client.set_curve_serverkey(server_cert.public_txt());
        client.set_linger(100);
        client.set_sndtimeo(100);
        client.connect(&format!("tcp://127.0.0.1:{}", port)).unwrap();

        sleep(Duration::from_millis(100));

        client.send_str("test").unwrap();
        assert!(server.recv_str().is_err());

        zauth.load_curve(None).unwrap();
        sleep(Duration::from_millis(100));

        client.connect(&format!("tcp://127.0.0.1:{}", port)).unwrap();
        sleep(Duration::from_millis(100));

        client.send_str("test").unwrap();
        assert_eq!(server.recv_str().unwrap().unwrap(), "test");

        let dir = TempDir::new("czmq_test").unwrap();
        client_cert.save_public(&format!("{}/testcert.txt", dir.path().to_str().unwrap())).unwrap();
        zauth.load_curve(dir.path().to_str()).unwrap();
        sleep(Duration::from_millis(100));

        client.connect(&format!("tcp://127.0.0.1:{}", port)).unwrap();
        sleep(Duration::from_millis(100));

        client.send_str("test").unwrap();
        assert_eq!(server.recv_str().unwrap().unwrap(), "test");
    }
}
