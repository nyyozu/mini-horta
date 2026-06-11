// PINOS — ajuste conforme seu hardware
const int sensorPin  = A0;
const int bombaPin   = 7;
const int luzPin     = 6;
const int ledAmarelo = 13;
const int ledVerde   = 12;

// PLANTA
enum Planta { MANJERICAO, SALSINHA, HORTELA, ALECRIM };
Planta plantaAtual = MANJERICAO;

// ── Controle de luz ───────────────────────────────────────────
bool luzLigada           = false;
bool manualOverride      = false;  // true = admin controlou manualmente, ignora ciclo automático
unsigned long luzLigouEm = 0;
unsigned long luzTotalHoje  = 0;
unsigned long ultimoToggleLuz = 0;

// ── Ciclo ─────────────────────────────────────────────────────
unsigned long ultimoCheck = 0;
const unsigned long intervalo = 10000;
bool bombaLigada   = false;
unsigned long bombaManualFim = 0; // millis() quando a irrigação manual termina (0 = sem timer)

// ─────────────────────────────────────────────────────────────

int lerUmidade() {
  return constrain(map(analogRead(sensorPin), 820, 420, 0, 100), 0, 100);
}

void getParametros(int &minU, int &maxU, String &nome) {
  switch (plantaAtual) {
    case MANJERICAO: minU = 60; maxU = 80; nome = "Manjericao"; break;
    case SALSINHA:   minU = 65; maxU = 80; nome = "Salsinha";   break;
    case HORTELA:    minU = 70; maxU = 85; nome = "Hortela";    break;
    case ALECRIM:    minU = 40; maxU = 60; nome = "Alecrim";    break;
  }
}

unsigned long getTempoLuzLigada() {
  switch (plantaAtual) {
    case SALSINHA:
    case HORTELA:  return 28800000UL; // 8h
    default:       return 43200000UL; // 12h
  }
}

unsigned long getTempoLuzDesligada() {
  return 43200000UL; // 12h
}

void ligarLuz() {
  if (!luzLigada) {
    luzLigada = true;
    luzLigouEm = millis();
    digitalWrite(luzPin, LOW);
  }
}

void desligarLuz() {
  if (luzLigada) {
    luzTotalHoje += millis() - luzLigouEm;
    luzLigada = false;
    luzLigouEm = 0;
    digitalWrite(luzPin, HIGH);
  }
}

unsigned long luzTotalSegundos() {
  unsigned long total = luzTotalHoje;
  if (luzLigada) total += millis() - luzLigouEm;
  return total / 1000UL;
}

void setup() {
  pinMode(sensorPin,  INPUT);
  pinMode(bombaPin,   OUTPUT);
  pinMode(luzPin,     OUTPUT);
  pinMode(ledAmarelo, OUTPUT);
  pinMode(ledVerde,   OUTPUT);

  Serial.begin(9600);

  digitalWrite(bombaPin,   HIGH);
  digitalWrite(luzPin,     HIGH);
  digitalWrite(ledAmarelo, HIGH);
  digitalWrite(ledVerde,   LOW);

  ligarLuz();
  ultimoToggleLuz = millis();
}

void loop() {
  unsigned long agora = millis();

  // ── Comandos do servidor ──────────────────────────────────
  if (Serial.available()) {
    String cmd = Serial.readStringUntil('\n');
    cmd.trim();
    if (cmd == "IRRIGAR" || cmd.startsWith("IRRIGAR ")) {
      // Aceita "IRRIGAR" (5s padrão) ou "IRRIGAR 30" (duração em segundos)
      int durSeg = 5;
      int espaco = cmd.indexOf(' ');
      if (espaco > 0) {
        durSeg = cmd.substring(espaco + 1).toInt();
        if (durSeg < 1 || durSeg > 300) durSeg = 5;
      }
      bombaLigada    = true;
      bombaManualFim = millis() + (unsigned long)durSeg * 1000UL;
    }
    if (cmd == "LUZ_ON") {
      manualOverride = true;
      ligarLuz();
      ultimoToggleLuz = agora; // reseta o timer do ciclo automático
    }
    if (cmd == "LUZ_OFF") {
      manualOverride = true;
      desligarLuz();
      ultimoToggleLuz = agora; // reseta o timer do ciclo automático
    }
    if (cmd == "LUZ_AUTO") {
      // Volta ao ciclo automático
      manualOverride = false;
      ultimoToggleLuz = agora;
    }
    if (cmd == "RESET_LUZ") {
      luzTotalHoje = 0;
    }
  }

  // ── Ciclo automático de luz (só se não estiver em override) ──
  if (!manualOverride) {
    if (luzLigada) {
      if (agora - ultimoToggleLuz >= getTempoLuzLigada()) {
        desligarLuz();
        ultimoToggleLuz = agora;
      }
    } else {
      if (agora - ultimoToggleLuz >= getTempoLuzDesligada()) {
        ligarLuz();
        ultimoToggleLuz = agora;
      }
    }
  }

  // Desliga bomba manual quando o timer expirar
  if (bombaManualFim > 0 && agora >= bombaManualFim) {
    bombaLigada    = false;
    bombaManualFim = 0;
  }

  // ── Leitura e envio ───────────────────────────────────────
  if (agora - ultimoCheck >= intervalo) {
    ultimoCheck = agora;

    int minU, maxU;
    String nome;
    getParametros(minU, maxU, nome);
    int umidade = lerUmidade();

    if (!bombaLigada && umidade < minU)        bombaLigada = true;
    if ( bombaLigada && umidade >= (minU + 5)) bombaLigada = false;

    if (bombaLigada) {
      digitalWrite(bombaPin, LOW);
      digitalWrite(ledAmarelo, LOW);
      digitalWrite(ledVerde, HIGH);
    } else {
      digitalWrite(bombaPin, HIGH);
      digitalWrite(ledAmarelo, HIGH);
      digitalWrite(ledVerde, LOW);
    }

    Serial.print("{\"plant_name\":\"");
    Serial.print(nome);
    Serial.print("\",\"humidity\":");
    Serial.print((float)umidade, 1);
    Serial.print(",\"light_lux\":");
    Serial.print(luzTotalSegundos());
    Serial.print(",\"luz_ligada\":");
    Serial.println(luzLigada ? 1 : 0);
  }
}
