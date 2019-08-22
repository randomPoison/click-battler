var uri = 'ws://' + location.host + '/chat';
var ws = new WebSocket(uri);

function message(data) {
    var line = document.createElement('p');
    line.innerText = data;
    chat.appendChild(line);
}

ws.onopen = function () {
    chat.innerHTML = "<p><em>Connected!</em></p>";
}

ws.onmessage = function (msg) {
    message(msg.data);
};

send.onclick = function () {
    var msg = text.value;
    ws.send(msg);
    text.value = '';

    message('<You>: ' + msg);
};
