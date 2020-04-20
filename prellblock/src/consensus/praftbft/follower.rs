use super::{
    super::{BlockHash, Body},
    message::ConsensusMessage,
    Error, PRaftBFT,
};
use pinxit::{PeerId, Signable, Signature, Signed};
use prellblock_client_api::Transaction;

impl PRaftBFT {
    fn handle_prepare_message(
        &self,
        peer_id: &PeerId,
        leader_term: usize,
        sequence_number: usize,
        block_hash: BlockHash,
    ) -> Result<ConsensusMessage, Error> {
        let mut follower_state = self.follower_state.lock().unwrap();
        follower_state.verify_message_meta(peer_id, leader_term, sequence_number)?;

        // All checks passed, update our state.
        follower_state.current_block_hash = block_hash;

        // Send AckPrepare to the leader.
        // *Note*: Technically, we only need to send a signature of
        // the PREPARE message.
        let ackprepare_message = ConsensusMessage::AckPrepare {
            leader_term: follower_state.leader_term,
            sequence_number,
            block_hash: follower_state.current_block_hash,
        };

        // Done :D
        Ok(ackprepare_message)
    }

    fn handle_append_message(
        &self,
        peer_id: &PeerId,
        leader_term: usize,
        sequence_number: usize,
        block_hash: BlockHash,
        ackprepare_signatures: Vec<(PeerId, Signature)>,
        data: Vec<Signed<Transaction>>,
    ) -> Result<ConsensusMessage, Error> {
        let mut follower_state = self.follower_state.lock().unwrap();
        follower_state.verify_message_meta(peer_id, leader_term, sequence_number)?;

        if block_hash != follower_state.current_block_hash {
            return Err(Error::ChangedBlockHash);
        }

        if sequence_number != follower_state.sequence + 1 {
            return Err(Error::WrongSequenceNumber);
        }

        // Check validity of ACKAPPEND Signatures.
        if !self.supermajority_reached(ackprepare_signatures.len()) {
            return Err(Error::NotEnoughSignatures);
        }
        for (peer_id, signature) in ackprepare_signatures {
            let ackprepare_message = ConsensusMessage::AckPrepare {
                leader_term,
                sequence_number,
                block_hash,
            };
            // Frage: Was tun bei faulty signature? Abbrechen oder weiter bei Supermajority?
            peer_id.verify(ackprepare_message, &signature)?;
        }

        // Check for transaction validity.
        for tx in data.clone() {
            tx.verify()?;
        }

        // TODO: Stateful validate transactions HERE.

        // Validate the Block Hash.
        let body = Body {
            height: follower_state.block_height + 1,
            prev_block_hash: follower_state.last_block_hash,
            transactions: data,
        };
        if block_hash != body.hash() {
            return Err(Error::WrongBlockHash);
        }

        follower_state.current_body = Some(body); // once told me the world was gonna roll me
                                                  // I ain't the sharpest tool in the sheeeed

        // ######################################################################################
        // #                                                                                    #
        // #                            ,.--------._                                            #
        // #                           /            ''.                                         #
        // #                         ,'                \     |"\                /\          /\  #
        // #                /"|     /                   \    |__"              ( \\        // ) #
        // #               "_"|    /           z#####z   \  //                  \ \\      // /  #
        // #                 \\  #####        ##------".  \//                    \_\\||||//_/   #
        // #                  \\/-----\     /          ".  \                      \/ _  _ \     #
        // #                   \|      \   |   ,,--..       \                    \/|(O)(O)|     #
        // #                   | ,.--._ \  (  | ##   \)      \                  \/ |      |     #
        // #                   |(  ##  )/   \ `-....-//       |///////////////_\/  \      /     #
        // #                     '--'."      \                \              //     |____|      #
        // #                  /'    /         ) --.            \            ||     /      \     #
        // #               ,..|     \.________/    `-..         \   \       \|     \ 0  0 /     #
        // #            _,##/ |   ,/   /   \           \         \   \       U    / \_//_/      #
        // #          :###.-  |  ,/   /     \        /' ""\      .\        (     /              #
        // #         /####|   |   (.___________,---',/    |       |\=._____|  |_/               #
        // #        /#####|   |     \__|__|__|__|_,/             |####\    |  ||                #
        // #       /######\   \      \__________/                /#####|   \  ||                #
        // #      /|#######`. `\                                /#######\   | ||                #
        // #     /++\#########\  \                      _,'    _/#########\ | ||                #
        // #    /++++|#########|  \      .---..       ,/      ,'##########.\|_||  Donkey By     #
        // #   //++++|#########\.  \.              ,-/      ,'########,+++++\\_\\ Hard'96       #
        // #  /++++++|##########\.   '._        _,/       ,'######,''++++++++\                  #
        // # |+++++++|###########|       -----."        _'#######' +++++++++++\                 #
        // # |+++++++|############\.     \\     //      /#######/++++ S@yaN +++\                #
        // #      ________________________\\___//______________________________________         #
        // #     / ____________________________________________________________________)        #
        // #    / /              _                                             _                #
        // #    | |             | |                                           | |               #
        // #     \ \            | | _           ____           ____           | |  _            #
        // #      \ \           | || \         / ___)         / _  )          | | / )           #
        // #  _____) )          | | | |        | |           (  __ /          | |< (            #
        // # (______/           |_| |_|        |_|            \_____)         |_| \_)           #
        // #                                                                           19.08.02 #
        // ######################################################################################
        // ───────────────────────────────────────
        // ───▐▀▄───────▄▀▌───▄▄▄▄▄▄▄─────────────
        // ───▌▒▒▀▄▄▄▄▄▀▒▒▐▄▀▀▒██▒██▒▀▀▄──────────
        // ──▐▒▒▒▒▀▒▀▒▀▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▀▄────────
        // ──▌▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▄▒▒▒▒▒▒▒▒▒▒▒▒▀▄──────
        // ▀█▒▒▒█▌▒▒█▒▒▐█▒▒▒▀▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▌─────
        // ▀▌▒▒▒▒▒▒▀▒▀▒▒▒▒▒▒▀▀▒▒▒▒▒▒▒▒▒▒▒▒▒▒▐───▄▄
        // ▐▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▌▄█▒█
        // ▐▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒█▒█▀─
        // ▐▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒█▀───
        // ▐▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▌────
        // ─▌▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▐─────
        // ─▐▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▌─────
        // ──▌▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▐──────
        // ──▐▄▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▒▄▌──────
        // ────▀▄▄▀▀▀▀▀▄▄▀▀▀▀▀▀▀▄▄▀▀▀▀▀▄▄▀────────

        let ackappend_message = ConsensusMessage::AckAppend {
            leader_term: follower_state.leader_term,
            sequence_number,
            block_hash: follower_state.current_block_hash,
        };
        Ok(ackappend_message)
    }

    fn handle_commit_message(
        &self,
        peer_id: &PeerId,
        leader_term: usize,
        sequence_number: usize,
        block_hash: BlockHash,
        ackappend_signatures: Vec<(PeerId, Signature)>,
    ) -> Result<ConsensusMessage, Error> {
        let mut follower_state = self.follower_state.lock().unwrap();
        follower_state.verify_message_meta(peer_id, leader_term, sequence_number)?;

        if block_hash != follower_state.current_block_hash {
            return Err(Error::ChangedBlockHash);
        }

        follower_state.last_block_hash = follower_state.current_block_hash;
        follower_state.sequence = sequence_number;
        follower_state.block_height += 1;

        // Write Blocks to BlockStorage
        unimplemented!();
    }

    /// Process the incoming `ConsensusMessages` (`PREPARE`, `ACKPREPARE`, `APPEND`, `ACKAPPEND`, `COMMIT`).
    pub fn handle_message(
        &self,
        message: Signed<ConsensusMessage>,
    ) -> Result<Signed<ConsensusMessage>, Error> {
        // Only RPUs are allowed.
        if !self.peers.contains_key(message.signer()) {
            return Err(Error::InvalidPeer(message.signer().clone()));
        }

        let message = message.verify()?;
        let peer_id = message.signer().clone();

        let response = match message.into_inner() {
            ConsensusMessage::Prepare {
                leader_term,
                sequence_number,
                block_hash,
            } => self.handle_prepare_message(&peer_id, leader_term, sequence_number, block_hash)?,
            ConsensusMessage::Append {
                leader_term,
                sequence_number,
                block_hash,
                ackprepare_signatures,
                data,
            } => self.handle_append_message(
                &peer_id,
                leader_term,
                sequence_number,
                block_hash,
                ackprepare_signatures,
                data,
            )?,
            ConsensusMessage::Commit {
                leader_term,
                sequence_number,
                block_hash,
                ackappend_signatures,
            } => self.handle_commit_message(
                &peer_id,
                leader_term,
                sequence_number,
                block_hash,
                ackappend_signatures,
            )?,
            _ => unimplemented!(),
        };

        let signed_response = response.sign(&self.identity).unwrap();
        Ok(signed_response)
    }
}
