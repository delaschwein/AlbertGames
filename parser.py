import os

GAMES_DIR = './games'

def main():
    files = os.listdir(GAMES_DIR)

    for ff in files:
        with open(f'{GAMES_DIR}/{ff}', 'r') as f:
            first_line = f.readline()
            print(first_line)
            continue
    pass

if __name__ == '__main__':
    main()
