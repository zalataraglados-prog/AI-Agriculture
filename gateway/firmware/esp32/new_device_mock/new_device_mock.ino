void setup() {
  Serial.begin(115200);
  delay(300);
  Serial.println("NEW_DEVICE boot");
}

void loop() {
  Serial.println("NEW_DEVICE alive");
  delay(2000);
}
