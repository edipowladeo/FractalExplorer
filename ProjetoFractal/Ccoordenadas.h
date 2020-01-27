#ifndef CCOORDENADAS_H
#define CCOORDENADAS_H



//TIPOS DE DADOS PERSONALIZADOS
//PASSAR PARA OUTRO ARQUIVO
typedef double TCoordPlano;
typedef float TCoordTela;

#include <SFML/Graphics.hpp>
#include <iostream>

using namespace std;



class CCoordenadas2DI
{
public:
    int x = 0;
    int y = 0;
    CCoordenadas2DI() {}
    CCoordenadas2DI(int x, int y) { this->x = x; this->y = y; }
    CCoordenadas2DI(const CCoordenadas2DI &C) { x = C.x; y = C.y; }
    void operator=(const CCoordenadas2DI &C) { x = C.x; y = C.y; }
    void operator+(const CCoordenadas2DI &C) { x += C.x; y += C.y; }
    void operator-(const CCoordenadas2DI &C) { x -= C.x; y -= C.y; }
    CCoordenadas2DI(sf::Vector2u vetor)
    {
        x = vetor.x;
        y = vetor.y;
    }
    // void operator*(const CCoordenadas2DI &C) {x -= C.x; y -= C.y; }
};



template <typename T> //funcionou
sf::Vector2<T> operator *(const sf::Vector2<T>& left, const sf::Vector2<T>& right)
{
    T X = left.x * right.x;
    T Y = left.y * right.y;
    return sf::Vector2<T>(X,Y);
}

template <typename T> // so funcionou passando (float)variavel
sf::Vector2<T> operator *(const sf::Vector2<T> &Vetor, T &scalar)
{
    T X = Vetor.x * scalar;
    T Y = Vetor.y * scalar;
    return sf::Vector2<T>(X, Y);
}


template <typename T, typename E> // Nao funcionou 
sf::Vector2<T> operator *(const sf::Vector2<T> &Vetor, E &scalar)
{
    T X = Vetor.x * scalar;
    T Y = Vetor.y * scalar;
    return sf::Vector2<T>(X, Y);
}



template <typename T> //nao funcionou
sf::Vector2<T> operator *(const sf::Vector2<T>& Vetor, double& scalar)
{
    T X = Vetor.x * scalar;
    T Y = Vetor.y * scalar;
    return sf::Vector2<T>(X,Y);
}


class IntervaloI{
    public:
int Min;
int Max;
};

class CCoordenadas2DPlano{
    public:
TCoordPlano x=0;
TCoordPlano y=0;
};

class CCoordenadas2DTela{
public:
TCoordTela x=0;
TCoordTela y=0;

CCoordenadas2DTela(TCoordTela x,TCoordTela y){
this->x = x;
this->y = y;
}

CCoordenadas2DTela(){}
CCoordenadas2DTela(sf::Vector2i vetor)
{
    x = vetor.x;
    y = vetor.y;
}
CCoordenadas2DTela(CCoordenadas2DI vetor)
{
    x = vetor.x;
    y = vetor.y;
}
};

class CIntervalo2DI
{
public:
    CCoordenadas2DI min, max;
    CIntervalo2DI() {}
    CIntervalo2DI(const CIntervalo2DI &C) { min = C.min; max = C.max; }
    void operator=(const CIntervalo2DI &C) { min = C.min; max = C.max; }
};




//deprecated
class Cvetor2d
{
    public:
    double x;
    double y;
};


class CcoordenadasPlano: public CCoordenadas2DPlano
{
public:
    double    delta=0;
CcoordenadasPlano(){}
CcoordenadasPlano(const CCoordenadas2DPlano &C) {x=C.x;y=C.y;}//copy constructor
void operator=(const CCoordenadas2DPlano &C ) {x=C.x;y=C.y;}//Assignment Operator overloading
void plot(){
cout << "C.x " << x << " C.y " << y << " delta " << delta;
}
};

bool operator ==(const CcoordenadasPlano& C1, const CcoordenadasPlano& C2);

sf::Vector2f operator*(const sf::Vector2f &Vetor, const float &escalar);

//CIntervalo2DI operator(const CIntervalo2DI &I1, const CIntervalo2DI &I2);

   
#endif

