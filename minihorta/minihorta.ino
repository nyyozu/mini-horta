// PINOS
const int sensorPin  = A0;
const int bombaPin   = 7;
const int ledAmarelo = 13;
const int ledVerde   = 12;
const int luzPin     = 6;

// PLANTA — mude plantaAtual para trocar a planta ativa
enum Planta { MANJERICAO, SALSINHA, HORTELA, ALECRIM };
Planta plantaAtual = MANJERICAO;

// TEMPO
unsigned long ultimoCheck = 0;
const unsigned long intervalo = 10000;

// LUZ
unsigned long ultimoToggleLuz = 0;
bool luzLigada = true;

// ESTADO
bool bombaLigada = false;

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

unsigned long getTempoLuz() {
  switch (plantaAtual) {
    case SALSINHA:
    case HORTELA:    return 28800000UL; // 8h
    default:         return 43200000UL; // 12h
  }
}

void setup() {
  pinMode(sensorPin,  INPUT);
  pinMode(bombaPin,   OUTPUT);
  pinMode(ledAmarelo, OUTPUT);
  pinMode(ledVerde,   OUTPUT);
  pinMode(luzPin,     OUTPUT);
  Serial.begin(9600);
  digitalWrite(bombaPin,   LOW);
  digitalWrite(ledVerde,   LOW);
  digitalWrite(ledAmarelo, HIGH);
  digitalWrite(luzPin,     LOW);
}

void loop() {
  unsigned long agora = millis();

  // Controle de luz
  if (agora - ultimoToggleLuz >= getTempoLuz()) {
    ultimoToggleLuz = agora;
    luzLigada = !luzLigada;
    digitalWrite(luzPin, luzLigada ? LOW : HIGH);
  }

  // Comando manual do servidor
  if (Serial.available()) {
    String cmd = Serial.readStringUntil('\n');
    cmd.trim();
    if (cmd == "IRRIGAR") bombaLigada = true;
  }

  // Ciclo de leitura
  if (agora - ultimoCheck >= intervalo) {
    ultimoCheck = agora;

    int minU, maxU;
    String nome;
    getParametros(minU, maxU, nome);
    int umidade = lerUmidade();

    if (!bombaLigada && umidade < minU)        bombaLigada = true;
    if ( bombaLigada && umidade >= (minU + 5)) bombaLigada = false;

    if (bombaLigada) {
      digitalWrite(bombaPin, HIGH); digitalWrite(ledAmarelo, LOW);  digitalWrite(ledVerde, HIGH);
    } else {
      digitalWrite(bombaPin, LOW);  digitalWrite(ledAmarelo, HIGH); digitalWrite(ledVerde, LOW);
    }

    // Envia JSON — usa plant_name, sem precisar de UUID
    Serial.print("{\"plant_name\":\"");
    Serial.print(nome);
    Serial.print("\",\"humidity\":");
    Serial.print((float)umidade, 1);
    Serial.println(",\"light_lux\":0.0}");
  }
}
