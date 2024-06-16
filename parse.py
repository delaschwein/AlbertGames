import os, json

GAMES_DIR = './results'

def main():
    files = os.listdir(GAMES_DIR)

    for ff in files:
        with open(f'{GAMES_DIR}/{ff}', 'r') as f:
            data = json.load(f)

if __name__ == '__main__':
    main()
