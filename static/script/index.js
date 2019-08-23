var app = new Vue({
    el: '#app',
    data: {
        connected: false,
        id: null,
        players: {},
    },

    methods: {
        healSelf: function () {
            // TODO: Send message to server to heal player.
        },
    }
});

var uri = 'ws://' + location.host + '/chat';
var ws = new WebSocket(uri);

ws.onopen = function () {
    app.connected = true;
};

ws.onmessage = function (msg) {
    let message = JSON.parse(msg.data);
    console.log(message);

    if (app.id === null) {
        console.log('Have not yet received ID, assuming this is the first message');
        app.id = message;
    } else if (message.type == null) {
        console.log('Message has no type, assuming this is the initial world state');
        app.players = message;
    } else {
        // We've finished initializing the client, so this should be a regular update.
        switch (message.type) {
            case 'WorldUpdate':
                app.players = message.players;
                break;

            case 'PlayerJoined':
                break;

            case 'PlayerDied':
                break;
        }
    }
};
