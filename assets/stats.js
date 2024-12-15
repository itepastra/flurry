const formatter = Intl.NumberFormat("en", { notation: "compact" });
function nString(value) {
	return formatter.format(value);
}

window.onload = function() {
	var client = document.getElementById("clientCounter");
	var pixel = document.getElementById("pixelCounter");
	var pixelAvg = document.getElementById("pixelCounterAvg");

	var pixelQueue = [];

	for (i = 0; i < 5; i++) {
		pixelQueue.push(0);
	}

	const stats = new WebSocket("/stats");

	stats.onopen = function() {
		console.log("Connected to flut-stats.");
	};
	stats.onerror = function(error) {
		console.error("An unknown error occured", error);
	};

	stats.onclose = function(event) {
		console.log("Server closed connection", event);
	};

	stats.onmessage = function(event) {
		const obj = JSON.parse(event.data);
		client.innerText = nString(obj.c);

		pixel.innerText = nString(obj.p);
		pixelQueue.push(obj.p);
		var old = pixelQueue.shift();
		pixelAvg.innerText = nString(obj.p - old);
	};
};
