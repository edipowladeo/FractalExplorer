#ifndef DEBUGOBJECTS_H
#define DEBUGOBJECTS_H

#include <SFML/Graphics.hpp>


#include <sstream>

#include "PropriedadesJanela.h"
#include "Ccoordenadas.h"

class CDebugObjects{
    public:
sf::RectangleShape RtgDesenhar;
sf::RectangleShape RtgAlocar;
sf::RectangleShape RtgDesalocar;
sf::RectangleShape RtgPivoTela;
sf::CircleShape Circulo_pivoTela;

sf::Font fonteTexto;
sf::Text textoDebug;
stringstream ssdebug;
string  stringDebug;


CDebugObjects();

CDebugObjects(CpropriedadesJanela & Propriedades);

void Atualizar(const CpropriedadesJanela &Propriedades);



};





#endif
