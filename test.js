const ws = new WebSocket("ws://127.0.0.1:3000/imgstream?canvas=0");
console.log("Connecting");
ws.onopen = () => {
    console.log("Connected");
}
ws.onmessage = (msg) => {
    var reader = new FileReader();
    reader.readAsDataURL(msg.data);
    reader.onloadend = function() {
        var base64data = reader.result;
        document.getElementById("image").src = base64data;
    }
}