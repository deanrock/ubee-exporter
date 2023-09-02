use prometheus_exporter::{self, prometheus::register_gauge_vec};
use scraper::{Html, Selector};

#[derive(Debug, PartialEq)]
struct DownstreamChannel {
    channel: u16,
    lock_status: String,
    modulation: String,
    frequency: f64,
    power: f64,
    snr: f64,
    symbol_rate: f64,
    correctables: f64,
    uncorrectables: f64,
}

#[derive(Debug, PartialEq)]
struct UpstreamChannel {
    channel: u16,
    lock_status: String,
    us_channel_type: String,
    frequency: f64,
    power: f64,
    symbol_rate: f64,
}

#[derive(Debug, PartialEq)]
struct ChannelData {
    downstream: Vec<DownstreamChannel>,
    upstream: Vec<UpstreamChannel>,
}

enum Direction {
    Upstream,
    Downstream,
}

pub fn exporter(server_host: String, server_port: u16, modem_host: String) {
    let binding = format!("{}:{}", server_host, server_port).parse().unwrap();
    let exporter = prometheus_exporter::start(binding).unwrap();

    let target_url = format!("http://{}/UbeeConnection.asp", modem_host);

    println!(
        "Listening on http://{}:{}/metrics, and scraping modem at {}",
        binding.ip(),
        binding.port(),
        target_url.clone(),
    );

    let downstream_labels = vec!["channel", "modulation", "lock_status"];
    let downstream_frequency = register_gauge_vec!(
        "ubee_channel_downstream_frequency",
        "Downstream channel frequency",
        &downstream_labels
    )
    .unwrap();
    let downstream_power = register_gauge_vec!(
        "ubee_channel_downstream_power",
        "Downstream channel power",
        &downstream_labels
    )
    .unwrap();
    let downstream_snr = register_gauge_vec!(
        "ubee_channel_downstream_snr",
        "Downstream channel snr",
        &downstream_labels
    )
    .unwrap();
    let downstream_symbol_rate = register_gauge_vec!(
        "ubee_channel_downstream_symbolrate",
        "Downstream channel symbolrate",
        &downstream_labels
    )
    .unwrap();
    let downstream_correctables = register_gauge_vec!(
        "ubee_channel_downstream_correctables",
        "Downstream channel correctables",
        &downstream_labels
    )
    .unwrap();
    let downstream_uncorrectables = register_gauge_vec!(
        "ubee_channel_downstream_uncorrectables",
        "Downstream channel uncorrectables",
        &downstream_labels
    )
    .unwrap();

    let upstream_labels = vec!["channel", "us_channel_type", "lock_status"];
    let upstream_frequency = register_gauge_vec!(
        "ubee_channel_upstream_frequency",
        "Upstream channel frequency",
        &upstream_labels
    )
    .unwrap();
    let upstream_power = register_gauge_vec!(
        "ubee_channel_upstream_power",
        "Upstream channel power",
        &upstream_labels
    )
    .unwrap();
    let upstream_symbol_rate = register_gauge_vec!(
        "ubee_channel_upstream_symbol_rate",
        "Upstream channel symbol rate",
        &upstream_labels
    )
    .unwrap();

    loop {
        let guard = exporter.wait_request();

        let body = reqwest::blocking::get(target_url.clone())
            .unwrap()
            .text()
            .unwrap();

        let data = parse_html(body);

        for channel in data.downstream {
            let channel_str = channel.channel.to_string();
            let label_values = vec![
                channel_str.as_str(),
                channel.modulation.as_str(),
                channel.lock_status.as_str(),
            ];
            downstream_frequency
                .with_label_values(&label_values)
                .set(channel.frequency);
            downstream_power
                .with_label_values(&label_values)
                .set(channel.power);
            downstream_snr
                .with_label_values(&label_values)
                .set(channel.snr);
            downstream_symbol_rate
                .with_label_values(&label_values)
                .set(channel.symbol_rate);
            downstream_correctables
                .with_label_values(&label_values)
                .set(channel.correctables);
            downstream_uncorrectables
                .with_label_values(&label_values)
                .set(channel.uncorrectables);
        }

        for channel in data.upstream {
            let channel_str = channel.channel.to_string();
            let label_values = vec![
                channel_str.as_str(),
                channel.us_channel_type.as_str(),
                channel.lock_status.as_str(),
            ];
            upstream_frequency
                .with_label_values(&label_values)
                .set(channel.frequency);
            upstream_power
                .with_label_values(&label_values)
                .set(channel.power);
            upstream_symbol_rate
                .with_label_values(&label_values)
                .set(channel.symbol_rate);
        }

        drop(guard);
    }
}

fn parse_html(html: String) -> ChannelData {
    let document = Html::parse_document(html.as_str());

    let table = Selector::parse("table").unwrap();
    let tr = Selector::parse("tr").unwrap();
    let td = Selector::parse("td").unwrap();

    let mut data = ChannelData {
        downstream: vec![],
        upstream: vec![],
    };

    for table in document.select(&table) {
        let mut trs = table.select(&tr);

        // First row determines channel direction.
        if let Some(x) = trs.next() {
            let m = match x.html().as_str() {
                x if x.contains("Downstream") => Some(Direction::Downstream),
                x if x.contains("Upstream") => Some(Direction::Upstream),
                _ => None,
            };

            // Skip the second row, which contains field labels.
            trs.next()
                .expect("second row should be present and contain field labels");

            for tr in trs {
                let tds: Vec<String> = tr.select(&td).map(|item| item.inner_html()).collect();

                if let Some(Direction::Downstream) = m {
                    assert!(tds.len() == 9, "downstream row should have 9 fields");
                    data.downstream.push(DownstreamChannel {
                        channel: tds[0].parse::<u16>().unwrap(),
                        lock_status: tds[1].clone(),
                        modulation: tds[2].clone(),
                        frequency: tds[3].split(" Hz").next().unwrap().parse::<f64>().unwrap(),
                        power: tds[4]
                            .split(" dBmV")
                            .next()
                            .unwrap()
                            .parse::<f64>()
                            .unwrap(),
                        snr: tds[5].split(" dB").next().unwrap().parse::<f64>().unwrap(),
                        symbol_rate: tds[6]
                            .split(" Ksym/sec")
                            .next()
                            .unwrap()
                            .parse::<f64>()
                            .unwrap(),
                        correctables: tds[7].parse::<f64>().unwrap(),
                        uncorrectables: tds[8].parse::<f64>().unwrap(),
                    });
                }

                if let Some(Direction::Upstream) = m {
                    assert!(tds.len() == 6, "upstream row should have 6 fields");
                    data.upstream.push(UpstreamChannel {
                        channel: tds[0].parse::<u16>().unwrap(),
                        lock_status: tds[1].clone(),
                        us_channel_type: tds[2].clone(),
                        symbol_rate: tds[3]
                            .split(" Ksym/sec")
                            .next()
                            .unwrap()
                            .parse::<f64>()
                            .unwrap(),
                        frequency: tds[4].split(" Hz").next().unwrap().parse::<f64>().unwrap(),
                        power: tds[5]
                            .split(" dBmV")
                            .next()
                            .unwrap()
                            .parse::<f64>()
                            .unwrap(),
                    });
                }
            }
        }
    }

    data
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn correctly_parses_html() {
        let html = r###"
        <table id="unrelated-table"></table>
        <table style="font-family: Helvetica;font-size:14">
            <tr bgcolor=#CE0000><th colspan=9><b><label id="ID_LABEL_TABLE_DOWNSTREAM">Downstream Bonded Channels</label></b></th></tr>
            <tr bgcolor="#FF8C00"><td><label id="ID_LABEL_TABLE_DOWNSTREAM_CHANNEL">Channel</label></td><td><label id="ID_LABEL_TABLE_DOWNSTREAM_LOCK_STATUS">Lock Status</label></td><td><label id="ID_LABEL_TABLE_DOWNSTREAM_MODULATION">Modulation</label></td><td><label id="ID_LABEL_TABLE_DOWNSTREAM_FREQUENCY">Frequency</label></td><td><label id="ID_LABEL_TABLE_DOWNSTREAM_POWER">Power</label></td><td><label id="ID_LABEL_TABLE_DOWNSTREAM_SNR">SNR</label></td><td><label id="ID_LABEL_TABLE_DOWNSTREAM_SYMBOL_RATE">Symbol Rate</label></td><td><label id="ID_LABEL_TABLE_DOWNSTREAM_CORRECTABLE">Correctables</label></td><td><label id="ID_LABEL_TABLE_DOWNSTREAM_UNCORRECTABLE">Uncorrectables</label></td></tr>
            <tr bgcolor="#9999CC"><td>1</td><td>Locked</td><td>QAM256</td><td>100000000 Hz</td><td>-1.0 dBmV</td><td>40.5 dB</td><td>1000 Ksym/sec</td><td>1</td><td>3</td></tr>
            <tr bgcolor="#99CCFF"><td>2</td><td>Locked</td><td>QAM256</td><td>200000000 Hz</td><td>-0.5 dBmV</td><td>40.0 dB</td><td>2000 Ksym/sec</td><td>2</td><td>2</td></tr>
            <tr bgcolor="#9999CC"><td>3</td><td>Locked</td><td>QAM256</td><td>300000000 Hz</td><td>-0.1 dBmV</td><td>41.6 dB</td><td>3000 Ksym/sec</td><td>3</td><td>1</td></tr>
        </table>
        <table style="font-family: Helvetica;font-size:14">
            <tr bgcolor=#CE0000><th colspan=7><b><label id="ID_LABEL_TABLE_UPSTREAM">Upstream Bonded Channels</label></b></th></tr>
            <tr bgcolor="#FF8C00"><td><label id="ID_LABEL_TABLE_UPSTREAM_CHANNEL">Channel</label></td><td><label id="ID_LABEL_TABLE_UPSTREAM_LOCK_STATUS">Lock Status</label></td><td><label id="ID_LABEL_TABLE_UPSTREAM_CHANNEL_TYPE">US Channel Type</label></td><td><label id="ID_LABEL_TABLE_UPSTREAM_SYMBOL_RATE">Symbol Rate</label></td><td><label id="ID_LABEL_TABLE_UPSTREAM_FREQUENCY">Frequency</label></td><td><label id="ID_LABEL_TABLE_UPSTREAM_POWER">Power</label></td></tr>
            <tr bgcolor="#9999CC"><td>1</td><td>Locked</td><td>ATDMA</td><td>1000 Ksym/sec</td><td>50000000 Hz</td><td>35.0 dBmV</td></tr>
            <tr bgcolor="#99CCFF"><td>2</td><td>Locked</td><td>ATDMA</td><td>2000 Ksym/sec</td><td>60000000 Hz</td><td>35.5 dBmV</td></tr>
            <tr bgcolor="#9999CC"><td>3</td><td>Locked</td><td>ATDMA</td><td>3000 Ksym/sec</td><td>70000000 Hz</td><td>35.8 dBmV</td></tr>
        </table>=
        "###;

        let data = parse_html(html.to_string());

        assert_eq!(
            data,
            ChannelData {
                downstream: vec![
                    DownstreamChannel {
                        channel: 1,
                        lock_status: "Locked".to_string(),
                        modulation: "QAM256".to_string(),
                        frequency: 100000000.0,
                        power: -1.0,
                        snr: 40.5,
                        symbol_rate: 1000.0,
                        correctables: 1.0,
                        uncorrectables: 3.0,
                    },
                    DownstreamChannel {
                        channel: 2,
                        lock_status: "Locked".to_string(),
                        modulation: "QAM256".to_string(),
                        frequency: 200000000.0,
                        power: -0.5,
                        snr: 40.0,
                        symbol_rate: 2000.0,
                        correctables: 2.0,
                        uncorrectables: 2.0,
                    },
                    DownstreamChannel {
                        channel: 3,
                        lock_status: "Locked".to_string(),
                        modulation: "QAM256".to_string(),
                        frequency: 300000000.0,
                        power: -0.1,
                        snr: 41.6,
                        symbol_rate: 3000.0,
                        correctables: 3.0,
                        uncorrectables: 1.0,
                    },
                ],
                upstream: vec![
                    UpstreamChannel {
                        channel: 1,
                        lock_status: "Locked".to_string(),
                        us_channel_type: "ATDMA".to_string(),
                        frequency: 50000000.0,
                        power: 35.0,
                        symbol_rate: 1000.0,
                    },
                    UpstreamChannel {
                        channel: 2,
                        lock_status: "Locked".to_string(),
                        us_channel_type: "ATDMA".to_string(),
                        frequency: 60000000.0,
                        power: 35.5,
                        symbol_rate: 2000.0,
                    },
                    UpstreamChannel {
                        channel: 3,
                        lock_status: "Locked".to_string(),
                        us_channel_type: "ATDMA".to_string(),
                        frequency: 70000000.0,
                        power: 35.8,
                        symbol_rate: 3000.0,
                    },
                ]
            }
        )
    }
}
