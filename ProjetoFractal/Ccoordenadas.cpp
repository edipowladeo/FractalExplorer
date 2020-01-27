#include "Ccoordenadas.h"

/*
sf::Vector2f operator*(const sf::Vector2f &Vetor, const float &escalar)
{
    return sf::Vector2f(Vetor.x * escalar, Vetor.y * escalar);
}*/


sf::Vector2f operator*(const sf::Vector2f &Vetor, const float &escalar)
{
    sf::Vector2f retorno;
    retorno.x = Vetor.x * escalar;
    retorno.y = Vetor.y * escalar;
    return retorno;
}

