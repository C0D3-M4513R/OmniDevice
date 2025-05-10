mod uuid;
mod ws;

pub use ws::ws_impl;
pub use uuid::get_devices;

#[rocket::get("/help")]
pub async fn help() -> &'static str {
    "Starting the websocket under ip/ws. Set one or multiply UUIDs by writing them after the hello message. \nThe last input can be a sampling rate. The default sampling Rate is 60 Sa/s.\nThe sampling Rate cant be higher than 100.000 Sa/s. Press enter to start the measurement."
}

/*
    // OPTIONS-Endpunkt, um Preflight-Anfragen zu behandeln
    CROW_ROUTE(crowApp, "/cors").methods("OPTIONS"_method)([]() {
        return crow::response(204); // Antwort ohne Inhalt
    });

    // Websocket
    CROW_WEBSOCKET_ROUTE(crowApp, "/ws")
    .onopen([&](crow::websocket::connection& conn) {
        websocketConnectionActive = true;
        CROW_LOG_INFO << "new websocket connection from " << conn.get_remote_ip();
        std::lock_guard<std::mutex> _(mtx);
        users.insert(&conn);
        conn.send_text("Hello, connection to websocket established. To start a measurement send the wished UUID, optional send a sampling rate between 10 and 100000");
    })
    .onclose([&](crow::websocket::connection& conn, const std::string& reason) {
        websocketConnectionActive = false;
        CROW_LOG_INFO << "websocket connection closed. Your measurement was stopped. " << reason;
        CloseWSConnection();
        std::lock_guard<std::mutex> _(mtx);
        users.erase(&conn);
    })
    .onmessage([&](crow::websocket::connection& conn, const std::string& data, bool is_binary) {
        CROW_LOG_INFO << "Received message: " << data;
        std::lock_guard<std::mutex> _(mtx);
        auto json_msg = nlohmann::json::parse(data); //TODO handle multiple
        if(json_msg.contains("command") && json_msg["command"].is_string()){
            const std::string cmd = json_msg["command"];
            if(cmd == "get_downsampled_in_range" && json_msg.contains("tmin") && json_msg.contains("tmax") && json_msg.contains("desired_number_of_samples")){
                conn.send_text(handle_get_downsampled_in_range(json_msg));
            }

        }/*
        auto measurement = std::make_shared<Measurement>(parseWSDataToMeasurement(data));
        if(!measurement->uuids.empty()) {
            clearAllDeques();

            wsDataQueueThread = std::thread(processDeque, std::ref(conn), measurement);
            wsDataQueueThreadActive = true;
            if(verbose) {
                conn.send_text("Sending data was started via the client.");
            }

            sendDataviaWSThread = std::thread(&Measurement::start, measurement);
            sendDataviaWSThreadActive = true;
            std::cout << "Measurement was set" << std::endl;
        }*/
    });

 */