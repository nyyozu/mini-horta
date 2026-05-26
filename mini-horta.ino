/* ******************** mini-horta — Rega Automática + Serial ********************
   Baseado no código original de Anderson Harayashiki Moreira (13.03.2019)
   Adaptado para integração com servidor mini-horta via serial JSON.

   Guia de conexão (igual ao original):
   LCD RS: pino 2
   LCD Enable: pino 3
   LCD D4: pino 4
   LCD D5: pino 5
   LCD D6: pino 6
   LCD D7: pino 7
   LCD R/W: GND
   LCD VSS: GND
   LCD VCC: VCC (5V)
   Potenciômetro 10K: GND / V0 (contraste) / VCC
   Sensor de umidade A0: pino A0
   Módulo Relé (válvula): pino 10   ← mesmo pino do original

   Protocolo serial (9600 bps):
   → Arduino envia a cada 5 s:
       {"plant_id":"<PLANT_ID>","humidity":42.0,"light_lux":0.0}
   → Servidor pode enviar "IRRIGAR\n" para acionar a rega manual
 ******************************************************************************** */

#include <LiquidCrystal.h>

// ── Pinos (idênticos ao original) ──────────────────────────────────────────────
const int rs = 2, en = 3, d4 = 4, d5 = 5, d6 = 6, d7 = 7;
LiquidCrystal lcd(rs, en, d4, d5, d6, d7);

const int pinoSensor  = A0;
const int pinoValvula = 10;

// ── Configuração — edite antes de gravar ───────────────────────────────────────

// UUID da planta cadastrada no servidor
const char PLANT_ID[] = "00000000-0000-0000-0000-000000000001";

// Limiar de umidade para rega automática (igual ao original: 50 %)
const int limiarSeco = 50;

// Duração da rega automática em ms (original: 5 s)
const unsigned long DURACAO_AUTO_MS   = 5000;
// Duração da rega manual (comando "IRRIGAR" do servidor)
const unsigned long DURACAO_MANUAL_MS = 5000;

// Intervalo de envio do JSON para o servidor
const unsigned long INTERVALO_SERIAL_MS = 5000;

// ── Estado ─────────────────────────────────────────────────────────────────────
int  umidadeSolo     = 0;
bool bombaLigada     = false;
unsigned long bombaLigadaEm = 0;
unsigned long duracaoBomba  = 0;
unsigned long ultimoEnvio   = 0;

// ── Setup ──────────────────────────────────────────────────────────────────────
void setup() {
    pinMode(pinoValvula, OUTPUT);
    digitalWrite(pinoValvula, HIGH);  // garante válvula fechada

    lcd.begin(16, 2);
    lcd.setCursor(0, 0);
    lcd.print("  mini-horta    ");
    lcd.setCursor(0, 1);
    lcd.print(" Inicializando  ");

    Serial.begin(9600);
    delay(1000);
}

// ── Loop ───────────────────────────────────────────────────────────────────────
void loop() {
    lerComandoSerial();
    gerenciarBomba();

    // Lê sensor e atualiza LCD a cada segundo
    static unsigned long ultimaLeitura = 0;
    if (millis() - ultimaLeitura >= 1000) {
        ultimaLeitura = millis();

        // Leitura e conversão idênticas ao original
        umidadeSolo = analogRead(pinoSensor);
        umidadeSolo = map(umidadeSolo, 1023, 0, 0, 100);

        atualizarLCD();
    }

    // Envia JSON para o servidor a cada INTERVALO_SERIAL_MS
    if (millis() - ultimoEnvio >= INTERVALO_SERIAL_MS) {
        ultimoEnvio = millis();
        enviarJSON();
        irrigacaoAutomatica();
    }
}

// ── LCD ────────────────────────────────────────────────────────────────────────
void atualizarLCD() {
    lcd.setCursor(0, 0);
    lcd.print("  Mini-Horta    ");
    lcd.setCursor(0, 1);

    if (bombaLigada) {
        lcd.print("    Regando     ");
    } else if (umidadeSolo >= limiarSeco) {
        lcd.print("Planta hidratada");
    } else {
        lcd.print("Umidade: ");
        lcd.print(umidadeSolo);
        lcd.print(" %    ");
    }
}

// ── Serial → servidor ──────────────────────────────────────────────────────────
void enviarJSON() {
    Serial.print("{\"plant_id\":\"");
    Serial.print(PLANT_ID);
    Serial.print("\",\"humidity\":");
    Serial.print(umidadeSolo);
    Serial.println(",\"light_lux\":0.0}");
}

// ── Serial ← servidor ──────────────────────────────────────────────────────────
void lerComandoSerial() {
    if (!Serial.available()) return;

    String cmd = Serial.readStringUntil('\n');
    cmd.trim();

    if (cmd == "IRRIGAR") {
        ligarBomba(DURACAO_MANUAL_MS);
    }
}

// ── Relé / bomba ───────────────────────────────────────────────────────────────
void ligarBomba(unsigned long duracao) {
    if (!bombaLigada) {
        digitalWrite(pinoValvula, LOW);
        bombaLigada   = true;
        bombaLigadaEm = millis();
        duracaoBomba  = duracao;
    }
}

void desligarBomba() {
    digitalWrite(pinoValvula, HIGH);
    bombaLigada = false;
}

// Desliga automaticamente após o tempo configurado
void gerenciarBomba() {
    if (bombaLigada && (millis() - bombaLigadaEm >= duracaoBomba)) {
        desligarBomba();
    }
}

// ── Lógica de rega automática (igual ao original) ─────────────────────────────
void irrigacaoAutomatica() {
    if (!bombaLigada && umidadeSolo < limiarSeco) {
        ligarBomba(DURACAO_AUTO_MS);
    }
}
