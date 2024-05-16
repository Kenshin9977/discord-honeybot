import os

import discord
from discord.ext import commands
from dotenv import load_dotenv

load_dotenv()
TOKEN = os.getenv('TOKEN')
HONEY_POT_CHANNEL_ID = int(os.getenv('HONEY_POT_CHANNEL_ID'))
LOG_CHANNEL_ID = int(os.getenv('LOG_CHANNEL_ID'))
CONTACT = os.getenv('CONTACT')

intents = discord.Intents.default()
intents.messages = True
intents.message_content = True
intents.dm_messages = True
intents.guilds = True
intents.members = True

bot = commands.Bot(command_prefix='!', intents=intents)


@bot.event
async def on_ready():
    print(f'Bot connecté en tant que {bot.user}')


@bot.event
async def on_message(message):
    if message.channel.id != HONEY_POT_CHANNEL_ID:
        return
    try:
        await message.author.send(
            f'''Vous avez été banni du serveur {message.guild.name} car '''
            '''vous avez posté un '''
            f'''message dans le canal restreint {message.channel.name}. '''
            '''Si vous pensez que ce ban est une erreur, vous pouvez '''
            f'''contacter {CONTACT}.'''
            )
        print(f'Message privé envoyé à {message.author}')

        await message.guild.ban(message.author, reason="Post dans le canal restreint")  # noqa
        print(
            f'''Utilisateur {message.author} banni pour avoir '''
            f'''posté dans {message.channel.name}'''
            )

        log_channel = bot.get_channel(LOG_CHANNEL_ID)
        if log_channel:
            await log_channel.send(
                f'''{message.author} a été banni pour avoir '''
                f'''posté dans {message.channel.mention}'''
                )
    except discord.Forbidden:
        print(f'Permissions insuffisantes pour bannir {message.author}')
        return
    except discord.HTTPException as e:
        print(
            f'''Erreur HTTP lors de la tentative de bannir ou d\'envoyer un '''
            f'''message privé à {message.author}: {e}'''
            )
bot.run(TOKEN)
