const { MessengerClient } = require('./grpc-web-out/messenger_grpc_web_pb.js');
const { MessengerRequest, Message } = require('./grpc-web-out/messenger_pb.js');

const client = new MessengerClient('http://localhost:8080', null, null);

window.onload = () => {
    console.log(document.children)

    const nicknameInput = document.getElementById('nickname-input');
    const nicknameBtn = document.getElementById('nickname-btn');
    const chatArea = document.getElementById('chat-area');
    const addresseeInput = document.getElementById('addressee-input');
    const messageInput = document.getElementById('message-input');
    const sendBtn = document.getElementById('send-btn');

    let nickname = '';

    nicknameBtn.addEventListener('click', () => {
        nickname = nicknameInput.value.trim();
        if (nickname) {
            nicknameInput.disabled = true;
            nicknameBtn.disabled = true;
            addresseeInput.disabled = false;
            messageInput.disabled = false;
            sendBtn.disabled = false;
            displayMessage(`You have joined the chat as ${nickname}`);
            receiveMessages(nickname);
        }
    });

    messageInput.addEventListener('keyup', (event) => {
        if (event.key === 'Enter') {
            sendMessage();
        }
    });

    sendBtn.addEventListener('click', sendMessage);

    function sendMessage() {
        const addressee = addresseeInput.value.trim();
        const content = messageInput.value.trim();
        if (content) {
            const message = new Message();
            message.setSender(nickname);
            message.setRecipient(addressee);
            message.setContent(content);

            client.sendMessage(message, {}, (err, response) => {
                if (err) {
                    console.error(err);
                } else {
                    console.log(response.getSuccess());
                }
            });
            displayMessage(`${nickname} to ${addressee}: ${content}`);
            messageInput.value = '';
        }
    }

    function displayMessage(message) {
        const messageElement = document.createElement('div');
        messageElement.textContent = message;
        chatArea.appendChild(messageElement);
        chatArea.scrollTop = chatArea.scrollHeight;
    }

    function receiveMessages(nickname) {
        const request = new MessengerRequest();
        request.setNickname(nickname);

        const stream = client.receiveMessages(request, {});
        stream.on('data', (message) => {
            displayMessage(`${message.getSender()}: ${message.getContent()}`);
        });
    }
};