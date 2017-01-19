use std::collections::VecDeque;
use net::msg::Message;
use net::pkt::Packet;
use err::Result;
use time::precise_time_ns;
use std;
use std::fmt::Debug;

#[derive(Clone)]
pub struct SentPacket {
    pub time: u64,
    pub seq: u32,
    pub packet: Packet,
}

impl Debug for SentPacket {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "SentPacket; time = {}, seq = {}", self.time, self.seq)
    }
}


#[derive(Clone)]
pub struct Connection {
    /// The sequence number of the next sent packet
    pub seq: u32,

    /// The first entry should always be Some.
    /// Some means that it's not yet acknowledged
    pub send_window: VecDeque<Option<SentPacket>>,
}
const RESEND_INTERVAL_MS: u64 = 1000;

impl<'a> Connection {
    pub fn new() -> Connection {
        Connection {
            seq: 0,
            send_window: VecDeque::new(),
        }
    }

    /// Returns Vec of encoded packets ready to be sent again
    pub fn get_resend_queue(&mut self) -> Vec<Vec<u8>> {
        let now = precise_time_ns();
        self.update_send_window();
        let mut result = Vec::new();
        for sent_packet in self.send_window.iter_mut() {
            if let &mut Some(ref mut sent_packet) = sent_packet {
                if now > sent_packet.time + RESEND_INTERVAL_MS * 1000000 {
                    sent_packet.time = now;
                    result.push(sent_packet.packet.encode());

                }
            }
        }
        result
    }


    pub fn acknowledge(&mut self, acked: u32) -> Result<()> {
        self.update_send_window();
        // Get the seq number of the first element
        let first_seq = match self.send_window.front() {
            None => bail!("Send window empty, but ack received."),
            Some(first) => {
                match first {
                    &Some(ref sent_packet) => sent_packet.seq,
                    &None => bail!("The first SentPacket is None."),
                }
            }
        };
        
        let index = (acked - first_seq) as usize;

        match self.send_window.get_mut(index) {
            Some(sent_packet) => *sent_packet = None,
            None => bail!("Index out of bounds: {}", index),
        };

        Ok(())
    }

    /// Removes all None's that appear at the front of the send window queue
    fn update_send_window(&mut self) {
        loop {
            let remove = match self.send_window.front() {Some(&None) => true, _ => false};
            if remove {
                self.send_window.pop_front();
            } else {
                break;
            }
        }
    }

    /// Wraps in a packet, encodes, and adds the packet to the send window queue. Returns the data
    /// enqueued.
    pub fn wrap_message(&mut self, msg: Message) -> Vec<u8> {
        let packet = Packet::Reliable {seq: self.seq, msg: msg};
        // debug!("Send"; "seq" => self.seq, "ack" => self.received+1);
        self.send_window.push_back(
            Some(SentPacket {
                time: precise_time_ns(),
                seq: self.seq,
                packet: packet.clone(),
            }));

        self.seq += 1;
        packet.encode()
    }

    /// Unwraps message from packet. If reliable, it will return Some(Packet) which should be sent
    /// as an acknowledgement.
    // Ideally, I would like to take a &[u8] here but it creates aliasing conflicts, as Socket will
    // have to send a slice of its own buffer.
    pub fn unwrap_message(&mut self, packet: Packet) -> Result<(Option<Message>, Option<Packet>)> {
        let mut received_msg = None;
        let mut ack_reply = None;
        match packet {
            Packet::Unreliable {msg} => {
                received_msg = Some(msg);
            },
            Packet::Reliable {seq, msg} => {
                received_msg = Some(msg);
                ack_reply = Some(Packet::Ack {ack: seq});
                info!("Recv"; "seq" => seq);
            },
            Packet::Ack {ack} => {
                self.acknowledge(ack)?;
                info!("Recv ack"; "ack" => ack);
            }
        };
        Ok((received_msg, ack_reply))
    }
}