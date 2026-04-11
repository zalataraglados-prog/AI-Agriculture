void setup() {
  Serial.begin(115200);
  delay(800);
  Serial.println("AIAG HELLO fw=probe baud=115200");
}

void loop() {
  Serial.println("AIAG HELLO fw=probe baud=115200");
  delay(2000);
}
