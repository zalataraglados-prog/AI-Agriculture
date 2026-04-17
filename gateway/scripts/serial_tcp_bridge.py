import socket
import serial
import threading
import sys

COM_PORT = 'COM5'
BAUD = 115200
LISTEN_HOST = '0.0.0.0'
LISTEN_PORT = 7000


def pump_socket_to_serial(sock, ser):
    try:
        while True:
            data = sock.recv(4096)
            if not data:
                break
            ser.write(data)
    except Exception:
        pass


def main():
    try:
        ser = serial.Serial(COM_PORT, BAUD, timeout=0.1)
    except Exception as e:
        print(f'[bridge] failed to open {COM_PORT}@{BAUD}: {e}')
        sys.exit(1)

    print(f'[bridge] serial opened: {COM_PORT}@{BAUD}')

    server = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    server.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
    server.bind((LISTEN_HOST, LISTEN_PORT))
    server.listen(1)
    print(f'[bridge] listening on {LISTEN_HOST}:{LISTEN_PORT}')

    while True:
        client, addr = server.accept()
        print(f'[bridge] client connected: {addr}')
        t = threading.Thread(target=pump_socket_to_serial, args=(client, ser), daemon=True)
        t.start()

        try:
            while True:
                data = ser.read(4096)
                if data:
                    client.sendall(data)
                else:
                    if t and not t.is_alive():
                        break
        except Exception as e:
            print(f'[bridge] connection ended: {e}')
        finally:
            try:
                client.close()
            except Exception:
                pass
            print('[bridge] client disconnected')


if __name__ == '__main__':
    main()

